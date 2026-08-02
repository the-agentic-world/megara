import { createHash, randomUUID } from "node:crypto";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import {
  runProcess,
  RPC_TIMEOUT_MS,
} from "./megara_process.js";

type JsonObject = Record<string, unknown>;
const PLANNING_MODEL_GUIDANCE =
  "Planning-only: use the returned next_action and current work item; never infer approval or invoke user-owned actions.";
function megaraCommand(): string {
  return process.env.MEGARA_BIN || "megara";
}

function newId(prefix: "req" | "cmd"): string {
  return `${prefix}_${randomUUID()}`;
}

export function stableCommandId(toolCallId: string): string {
  return `cmd_pi_${createHash("sha256").update(toolCallId, "utf8").digest("hex")}`;
}

function rpcRequest(
  operation: string,
  values: JsonObject,
  mutation: boolean,
  commandId: string | undefined = undefined,
): JsonObject {
  const { session_id, expected_revision, ...params } = values;
  return {
    protocol_version: 1,
    request_id: newId("req"),
    operation,
    ...(mutation ? { command_id: commandId || newId("cmd") } : {}),
    ...(session_id === undefined ? {} : { session_id }),
    ...(expected_revision === undefined ? {} : { expected_revision }),
    ...(Object.keys(params).length === 0 ? {} : { params }),
  };
}

async function runPlanningRpc(
  cwd: string,
  request: JsonObject,
  signal: AbortSignal | undefined,
): Promise<JsonObject> {
  const result = await runProcess(
    cwd,
    ["planning", "rpc", "--project", cwd],
    JSON.stringify(request) + "\n",
    signal,
  );
  const lines = result.stdout.trim().split(/\r?\n/).filter(Boolean);
  if (lines.length === 1) {
    try {
      return JSON.parse(lines[0]) as JsonObject;
    } catch (error) {
      if (result.code === 0) {
        throw new Error(`IO_ERROR: invalid planning rpc response: ${String(error)}`);
      }
    }
  }
  throw new Error(`IO_ERROR: ${result.stderr.trim() || `Megara planning rpc exited with ${result.code}`}`);
}

function requireOk(response: JsonObject): JsonObject {
  if (response.ok === false) {
    const error = response.error as JsonObject | undefined;
    throw new Error(`${String(error?.code || "INVALID_REQUEST")}: ${String(error?.message || "Planning request failed")}`);
  }
  return response;
}

export function transportErrorResponse(request: JsonObject, error: unknown): JsonObject {
  const message = error instanceof Error ? error.message : String(error);
  return {
    protocol_version: 1,
    request_id: request.request_id,
    operation: request.operation,
    ok: false,
    error: {
      code: "IO_ERROR",
      message,
      retryable: false,
      details: { transport: "pi", cause: message },
    },
  };
}


function responseText(response: JsonObject): string {
  return JSON.stringify(response);
}

function projectTool(
  pi: ExtensionAPI,
  name: string,
  operation: string,
  parameters: unknown,
  mutation: boolean,
): void {
  pi.registerTool({
    name,
    label: `Megara ${operation}`,
    description: `Run the typed ${operation} planning operation once. ${PLANNING_MODEL_GUIDANCE}`,
    parameters,
    async execute(toolCallId, params, signal, _onUpdate, ctx) {
      const commandId = mutation
        ? stableCommandId(String(toolCallId))
        : undefined;
      const request = rpcRequest(operation, params as JsonObject, mutation, commandId);
      let response: JsonObject;
      try {
        response = await runPlanningRpc(ctx.cwd, request, signal);
      } catch (error) {
        response = transportErrorResponse(request, error);
      }
      return { content: [{ type: "text", text: responseText(response) }], details: response };
    },
  });
}

function candidateDetails(state: JsonObject, kind: "spec" | "plan"): JsonObject {
  const track = state[kind] as JsonObject | undefined;
  const candidate = track?.current_candidate as JsonObject | undefined;
  if (!candidate) throw new Error(`no current ${kind} candidate`);
  return {
    candidate_id: candidate.candidate_id,
    semantic_hash: candidate.semantic_hash,
    base_revision: kind === "spec" ? candidate.base_domain_revision : candidate.base_plan_revision,
  };
}

async function currentCandidate(
  cwd: string,
  session: string,
  signal: AbortSignal | undefined,
  kind: "spec" | "plan",
): Promise<{ revision: number; details: JsonObject }> {
  const response = requireOk(await runPlanningRpc(
    cwd,
    rpcRequest("planning.status", { session_id: session }, false),
    signal,
  ));
  const state = response.result && (response.result as JsonObject).state as JsonObject;
  return {
    revision: Number(response.revision),
    details: candidateDetails(state, kind),
  };
}

async function confirmAndExecute(
  pi: ExtensionAPI,
  ctx: ExtensionContext,
  kind: "spec" | "plan" | "purge",
  action: "approve" | "revise" | "purge",
  args: string,
): Promise<void> {
  const parts = args.trim().split(/\s+/).filter(Boolean);
  const session = parts.shift();
  if (!session || parts.length > 0) {
    ctx.ui.notify(
      `Usage: /megara-${action} ${kind} <session-id>`,
      "warning",
    );
    return;
  }
  let revision: number;
  let command: string[];
  let summary: JsonObject;
  if (kind === "purge") {
    const status = requireOk(await runPlanningRpc(
      ctx.cwd,
      rpcRequest("planning.status", { session_id: session }, false),
      undefined,
    ));
    revision = Number(status.revision);
    summary = { session_id: session, revision };
    command = [
      "planning", "purge", "--project", ctx.cwd, "--session", session,
      "--expected-revision", String(revision), "--confirm", session, "--json",
    ];
  } else {
    const current = await currentCandidate(ctx.cwd, session, undefined, kind);
    revision = current.revision;
    summary = { session_id: session, revision, ...current.details };
    const artifactKind = kind === "spec" ? "spec" : "plan";
    if (action === "revise") {
      if (!ctx.hasUI) throw new Error("Megara user input requires the Pi UI");
      const text = await ctx.ui.input(
        `Megara ${kind} revision request`,
        "Describe the requested revision",
      );
      if (!text?.trim()) {
        ctx.ui.notify("Revision cancelled.", "info");
        return;
      }
      summary = { ...summary, revision_text: text };
      command = ["planning", artifactKind, "revise", "--text", text];
    } else {
      command = ["planning", artifactKind, "approve"];
    }
    command.push(
      "--project", ctx.cwd, "--session", session, "--expected-revision", String(revision),
      "--candidate", String(summary.candidate_id),
    );
    if (action === "approve") {
      command.push("--semantic-hash", String(summary.semantic_hash));
      command.push(kind === "spec" ? "--base-domain-revision" : "--base-plan-revision");
      command.push(String(summary.base_revision));
    }
    command.push("--json");
  }
  if (!ctx.hasUI) throw new Error("Megara user confirmation requires the Pi UI");
  const exactCommand = [megaraCommand(), ...command];
  const approved = await ctx.ui.confirm(
    `Megara ${kind} ${action} confirmation`,
    `${JSON.stringify(summary)}\nargv: ${JSON.stringify(exactCommand)}\nExecute the exact user command?`,
  );
  if (!approved) return;
  const result = await pi.exec(megaraCommand(), command, { cwd: ctx.cwd, timeout: RPC_TIMEOUT_MS });
  if (result.code !== 0) throw new Error(result.stderr || result.stdout || `Megara ${kind} failed`);
  ctx.ui.notify(result.stdout.trim() || `Megara ${kind} complete.`, "info");
}

export default function (pi: ExtensionAPI) {
  const string = Type.String();
  const session = Type.String();
  const revision = Type.Integer({ minimum: 0 });
  const enumValue = (values: string[]) => Type.Union(values.map((value) => Type.Literal(value)));
  const noCommand = (properties: Record<string, unknown>, required: string[] = []) =>
    Type.Object(properties, { additionalProperties: false, required });

  projectTool(pi, "planning_start", "planning.start", noCommand({ request: string, title: Type.Optional(string) }, ["request"]), true);
  projectTool(pi, "planning_answer", "planning.answer", noCommand({ session_id: session, expected_revision: revision, question_id: string, text: string, selected_choice_ids: Type.Optional(Type.Array(string)) }, ["session_id", "expected_revision", "question_id", "text"]), true);
  projectTool(pi, "planning_status", "planning.status", noCommand({ session_id: Type.Optional(session) }), false);
  projectTool(pi, "planning_current", "planning.current", noCommand({ session_id: Type.Optional(session) }), false);
  projectTool(pi, "planning_list", "planning.list", noCommand({ phase: Type.Optional(enumValue(["interview", "specification", "planning", "complete"])) }), false);
  projectTool(pi, "planning_evidence_refresh", "planning.evidence.refresh", noCommand({ session_id: session, expected_revision: revision, citations: Type.Array(Type.Object({}, { additionalProperties: true })) }, ["session_id", "expected_revision", "citations"]), true);
  projectTool(pi, "planning_audit_apply", "planning.audit.apply", noCommand({ session_id: session, expected_revision: revision, mode: enumValue(["delta", "full"]), proposal: Type.Object({}, { additionalProperties: true }) }, ["session_id", "expected_revision", "mode", "proposal"]), true);
  projectTool(pi, "planning_spec_generate", "planning.spec.generate", noCommand({ session_id: session, expected_revision: revision, proposal: Type.Object({}, { additionalProperties: true }), projection_policy: Type.Optional(Type.Object({}, { additionalProperties: true })) }, ["session_id", "expected_revision", "proposal"]), true);
  projectTool(pi, "planning_spec_show", "planning.spec.show", noCommand({ session_id: Type.Optional(session), candidate_id: Type.Optional(string), format: Type.Optional(enumValue(["markdown", "json"])) }), false);
  projectTool(pi, "planning_plan_generate", "planning.plan.generate", noCommand({ session_id: session, expected_revision: revision, proposal: Type.Object({}, { additionalProperties: true }), projection_policy: Type.Optional(Type.Object({}, { additionalProperties: true })) }, ["session_id", "expected_revision", "proposal"]), true);
  projectTool(pi, "planning_plan_show", "planning.plan.show", noCommand({ session_id: Type.Optional(session), candidate_id: Type.Optional(string), format: Type.Optional(enumValue(["markdown", "json"])) }), false);

  pi.registerCommand("megara-approve", {
    description: "Confirm and approve the current Megara spec or plan candidate.",
    handler: async (args, ctx) => {
      const [kind, ...rest] = args.trim().split(/\s+/);
      if (kind !== "spec" && kind !== "plan") {
        ctx.ui.notify("Usage: /megara-approve spec|plan <session-id>", "warning");
        return;
      }
      await confirmAndExecute(pi, ctx, kind, "approve", rest.join(" "));
    },
  });
  pi.registerCommand("megara-revise", {
    description: "Confirm and revise the current Megara spec or plan candidate.",
    handler: async (args, ctx) => {
      const [kind, ...rest] = args.trim().split(/\s+/);
      if (kind !== "spec" && kind !== "plan") {
        ctx.ui.notify("Usage: /megara-revise spec|plan <session-id>", "warning");
        return;
      }
      await confirmAndExecute(pi, ctx, kind, "revise", rest.join(" "));
    },
  });
  pi.registerCommand("megara-purge", {
    description: "Confirm and purge one Megara planning session.",
    handler: async (args, ctx) =>
      confirmAndExecute(pi, ctx, "purge", "purge", args),
  });
}
