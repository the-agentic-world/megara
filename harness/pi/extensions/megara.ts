import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

type Workflow = "deep-interview" | "ralplan" | "ultragoal" | "team";
type ActiveWorkflow = { workflow: Workflow; eventId: string };
type ProcessResult = { code: number; stdout: string; stderr: string };

const WORKFLOW_PATTERN = /(?:\$|\/skill:)(deep-interview|ralplan|ultragoal|team)\b/;
const MAX_OUTPUT_BYTES = 50 * 1024;
const RETRY_DELAYS = [1_000, 2_000];

function megaraCommand(): string {
  return process.env.MEGARA_BIN || "megara";
}

function projectScope(cwd: string): "project" | "global" {
  return existsSync(join(cwd, ".pi", "extensions", "megara.ts")) ? "project" : "global";
}

function agentPath(cwd: string, role: string): string {
  const project = join(cwd, ".pi", "agents", `${role}.md`);
  if (existsSync(project)) return project;
  const global = join(process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent"), "agents", `${role}.md`);
  if (existsSync(global)) return global;
  throw new Error(`Megara role agent is unavailable: ${role}`);
}

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function runtimeEvent(cwd: string, payload: Record<string, unknown>): Promise<Record<string, any>> {
  return new Promise((resolve, reject) => {
    const child = spawn(megaraCommand(), ["pi", "event", "--scope", projectScope(cwd)], {
      cwd,
      shell: false,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
    child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) return reject(new Error(stderr.trim() || `Megara runtime exited with ${code}`));
      try { resolve(JSON.parse(stdout)); } catch { reject(new Error("Megara runtime returned invalid JSON")); }
    });
    child.stdin.end(JSON.stringify({ protocol_version: 1, ...payload }));
  });
}

function readFinalOutput(jsonl: string): string {
  let output = "";
  for (const line of jsonl.split("\n")) {
    try {
      const event = JSON.parse(line);
      if (event.type === "message_end" && event.message?.role === "assistant") {
        output = event.message.content?.find((part: { type: string }) => part.type === "text")?.text || output;
      }
    } catch { /* Ignore malformed streaming lines. */ }
  }
  return output.slice(0, MAX_OUTPUT_BYTES);
}

function executionFailure(jsonl: string): string | undefined {
  for (const line of jsonl.split("\n")) {
    try {
      const event = JSON.parse(line);
      const message = event.type === "message_end" ? event.message : undefined;
      if (message?.role === "assistant" && (message.stopReason === "error" || message.stopReason === "aborted")) {
        return message.errorMessage || `Pi role stopped: ${message.stopReason}`;
      }
    } catch { /* Ignore malformed streaming lines. */ }
  }
  return undefined;
}

async function runRole(cwd: string, role: string, task: string, model: string | undefined, signal: AbortSignal | undefined): Promise<ProcessResult> {
  const args = ["--mode", "json", "-p", "--no-session", "--approve", "--append-system-prompt", agentPath(cwd, role)];
  if (model) args.push("--model", model);
  args.push(`Task: ${task}`);
  return new Promise((resolve) => {
    const child = spawn("pi", args, { cwd, shell: false, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    const terminate = () => {
      child.kill("SIGTERM");
      setTimeout(() => { if (!child.killed) child.kill("SIGKILL"); }, 5_000);
    };
    const timeout = setTimeout(terminate, 120_000);
    if (signal?.aborted) terminate();
    else signal?.addEventListener("abort", terminate, { once: true });
    child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
    child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
    child.on("close", (code) => {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", terminate);
      resolve({ code: code ?? 1, stdout, stderr });
    });
    child.on("error", (error) => {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", terminate);
      resolve({ code: 1, stdout, stderr: `${stderr}${error.message}` });
    });
  });
}

export default function (pi: ExtensionAPI) {
  let active: ActiveWorkflow | undefined;

  pi.on("session_start", async (_event, ctx) => {
    for (const entry of ctx.sessionManager.getEntries()) {
      if (entry.type === "custom" && entry.customType === "megara-pi-workflow") active = entry.data as ActiveWorkflow;
    }
  });

  pi.on("input", async (event, ctx) => {
    const match = event.text.match(WORKFLOW_PATTERN);
    if (!match) return;
    const next = { workflow: match[1] as Workflow, eventId: randomUUID() };
    const activation = await runtimeEvent(ctx.cwd, { action: "activate", event_id: next.eventId, workflow: next.workflow });
    if (activation.status === "blocked") throw new Error(String(activation.message || "Megara blocked project role agents"));
    active = next;
    pi.appendEntry("megara-pi-workflow", active);
  });

  pi.on("before_agent_start", async (event, ctx) => {
    if (!active) return;
    const response = await runtimeEvent(ctx.cwd, { action: "next-action", event_id: active.eventId, workflow: active.workflow });
    const roles = Array.isArray(response.required_roles) ? response.required_roles.join(", ") : "";
    const roleInstruction = roles
      ? ` Delegate focused work with megara_subagent and wait for each result: ${roles}.`
      : "";
    return {
      systemPrompt: `${event.systemPrompt}\n\n[MEGARA WORKFLOW]\nThe active workflow is ${active.workflow}. Follow the loaded workflow skill; do not replace it with a free-form plan.${roleInstruction}`,
    };
  });

  pi.registerTool({
    name: "megara_subagent",
    label: "Megara subagent",
    description: "Run one trusted Megara role agent in an isolated Pi process; output is capped at 50 KB.",
    promptGuidelines: ["Use megara_subagent for each required Megara workflow role before advancing the workflow."],
    parameters: Type.Object({ role: Type.String(), task: Type.String(), model: Type.Optional(Type.String()) }),
    async execute(_id, params, signal, _onUpdate, ctx) {
      if (!active) throw new Error("Megara workflow is not active");
      let requestedModel = params.model;
      let fallbackUsed = false;
      for (;;) {
        const prepared = await runtimeEvent(ctx.cwd, {
          action: "prepare-attempt", event_id: active.eventId, workflow: active.workflow, role: params.role, model: requestedModel,
        });
        if (prepared.status === "completed") {
          return { content: [{ type: "text", text: String(prepared.output || "completed") }] };
        }
        if (prepared.status === "blocked") throw new Error(String(prepared.message || "Megara blocked this role"));
        const attemptId = String(prepared.attempt_id);
        const result = await runRole(ctx.cwd, params.role, params.task, requestedModel || prepared.model, signal);
        const failure = executionFailure(result.stdout);
        const completed = await runtimeEvent(ctx.cwd, {
          action: "attempt-finished", event_id: active.eventId, attempt_id: attemptId, workflow: active.workflow,
          role: params.role, status: result.code === 0 && !failure ? "completed" : "failed", output: readFinalOutput(result.stdout),
          error: failure || result.stderr || `Pi role exited with ${result.code}`,
        });
        if (completed.status === "completed") {
          return { content: [{ type: "text", text: String(completed.output || readFinalOutput(result.stdout) || "completed") }] };
        }
        if (completed.status === "retry") {
          const delay = Number(completed.retry_after_ms || RETRY_DELAYS[1]);
          await sleep(delay);
          continue;
        }
        if (completed.status === "fallback" && !fallbackUsed) {
          fallbackUsed = true;
          requestedModel = undefined;
          continue;
        }
        throw new Error(String(completed.message || result.stderr || "Megara role failed"));
      }
    },
  });

  pi.on("session_shutdown", async (_event, ctx) => {
    if (!active) return;
    await runtimeEvent(ctx.cwd, { action: "shutdown", event_id: active.eventId, workflow: active.workflow });
  });
}
