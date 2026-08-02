import { spawn } from "node:child_process";

export type ProcessResult = { code: number | null; stdout: string; stderr: string };
export type ProcessPolicy = {
  timeoutMs: number;
  terminationGraceMs: number;
};

export const MAX_RPC_OUTPUT_BYTES = 4 * 1024 * 1024;
export const RPC_TIMEOUT_MS = 120_000;
export const TERMINATION_GRACE_MS = 2_000;

function megaraCommand(): string {
  return process.env.MEGARA_BIN || "megara";
}

export function runProcess(
  cwd: string,
  args: string[],
  input: string,
  signal: AbortSignal | undefined,
): Promise<ProcessResult> {
  return runProcessWithPolicy(cwd, args, input, signal, {
    timeoutMs: RPC_TIMEOUT_MS,
    terminationGraceMs: TERMINATION_GRACE_MS,
  });
}

export function runProcessWithPolicy(
  cwd: string,
  args: string[],
  input: string,
  signal: AbortSignal | undefined,
  policy: ProcessPolicy,
): Promise<ProcessResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(megaraCommand(), args, {
      cwd,
      shell: false,
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let settled = false;
    let stopping = false;
    let failure: Error | undefined;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    let grace: ReturnType<typeof setTimeout> | undefined;
    const terminate = () => {
      if (stopping) return;
      stopping = true;
      child.kill("SIGTERM");
      grace = setTimeout(() => child.kill("SIGKILL"), policy.terminationGraceMs);
    };
    const abort = () => {
      failure ||= new Error("IO_ERROR: Megara planning rpc was aborted");
      terminate();
    };
    const finish = () => {
      if (settled) return;
      settled = true;
      if (timeout) clearTimeout(timeout);
      if (grace) clearTimeout(grace);
      signal?.removeEventListener("abort", abort);
      if (failure) reject(failure);
      else {
        resolve({
          code: child.exitCode,
          stdout: Buffer.concat(stdoutChunks).toString("utf8"),
          stderr: Buffer.concat(stderrChunks).toString("utf8"),
        });
      }
    };
    timeout = setTimeout(() => {
      failure = new Error("IO_ERROR: Megara planning rpc timed out");
      terminate();
    }, policy.timeoutMs);
    if (signal?.aborted) abort();
    else signal?.addEventListener("abort", abort, { once: true });
    child.stdout.on("data", (chunk: Buffer) => {
      const bytes = Buffer.from(chunk);
      if (stdoutBytes + bytes.byteLength > MAX_RPC_OUTPUT_BYTES) {
        failure = new Error("IO_ERROR: Megara planning rpc response exceeded 4 MiB");
        terminate();
        return;
      }
      stdoutBytes += bytes.byteLength;
      stdoutChunks.push(bytes);
    });
    child.stderr.on("data", (chunk: Buffer) => {
      const bytes = Buffer.from(chunk);
      if (stderrBytes + bytes.byteLength > MAX_RPC_OUTPUT_BYTES) {
        failure = new Error("IO_ERROR: Megara planning rpc stderr exceeded 4 MiB");
        terminate();
        return;
      }
      stderrBytes += bytes.byteLength;
      stderrChunks.push(bytes);
    });
    child.on("error", (error) => {
      failure ||= new Error(`IO_ERROR: ${error.message}`);
      terminate();
    });
    child.stdin.on("error", (error) => {
      failure ||= new Error(`IO_ERROR: planning rpc stdin: ${error.message}`);
      terminate();
    });
    child.on("close", () => finish());
    child.stdin.end(input);
  });
}
