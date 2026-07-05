# Megara Harness Source

This directory is the source of truth for Megara's bundled harness.

The installer compiles these files into the `megara` binary, writes them to the selected install scope, and projects them into supported agent runtimes.

## Configuration

- `megara.toml` is the SSOT harness configuration.
- `locale` controls user-facing response language in projected runtimes.
- Technical literals such as file paths, commands, package names, config keys, and quoted source text stay unchanged even when prose follows the configured locale.
- The locale rule covers progress updates, clarification questions, option labels, plans, verification notes, and final summaries.
- Structured block keys stay parseable, while free-text block values should follow the configured locale unless they are technical literals.

## Workflows

- `deep-interview`: Socratic clarification before planning.
- `ralplan`: consensus planning with planner, architect, and critic roles.
- `ultragoal`: durable goal execution with verification gates.
- `team`: multi-agent lane coordination.

## Skills

- `caveman`: terse response compression mode adapted from `juliusbrussee/caveman`. It is installed as a Megara skill and listed as a default active skill.
- `insane-search`: on-demand skill wrapper for the bundled `fivetaku/insane-search` tool. It is installed as a skill so users can invoke `$insane-search`, but it is not a default active skill.

## Tools

- `insane-search`: on-demand public web access helper adapted from `fivetaku/insane-search`.
  - The executable tool files are installed under `tools/insane-search`.
  - The matching skill under `skills/insane-search` is only a trigger and usage guide; the engine and references stay under `tools/insane-search`.
  - The wrapper bootstraps Python dependencies into `state/tools/insane-search/venv` on first use.
  - Runtime agents should read `tools/insane-search/TOOL.md` and use `bin/insane-search` only when normal search/fetch paths fail or a blocked/JS-heavy public page needs stronger access.

## Agents

- `executor`
- `planner`
- `architect`
- `critic`

## Runtime Hooks

- `megara hook`: portable Rust hook runner used by runtime adapters to keep lightweight event state without breaking the agent runtime.
- Project-scope Codex installs keep skills under `.agents/skills`; Codex App reads that directory directly. Megara does not mirror those skills into `.codex/skills`, because doing so makes the same skill appear twice.
- Codex hook payloads do not expose a stable `surface` field for CLI/App detection. For Codex, Megara should infer surface from the session log referenced by `transcript_path` before falling back to prompt-shape heuristics.
  - `session_meta.payload.source == "exec"` indicates a `codex exec` CLI session in observed Codex `0.142.5` payloads.
  - `session_meta.payload.source == "vscode"` with `thread_source == "subagent"` and a `<codex_delegation><input>...</input>` prompt wrapper indicates a Codex App delegated thread in observed Codex `0.142.5` payloads.
  - Prompt and workflow detection must run on the effective user prompt. For delegated Codex App payloads, extract the text inside `<input>...</input>` before checking slash commands or Megara skill triggers.
- Codex subagent spawning is a model-turn capability, not a hook command capability. Hooks can observe `SubagentStart`/`SubagentStop` and can add context or block a turn, but they must not assume they can directly call Codex's internal spawn tools from a hook process.
- Hook state is append-only by default:
  - `.agents/state/hooks/events.jsonl` indexes every hook event.
  - `.agents/state/hooks/payloads/<runtime>/<event>/*.json` stores every raw payload.
  - `.agents/state/hooks/conversation-events.jsonl` indexes user prompts and assistant stop messages.
  - `.agents/state/hooks/conversation.jsonl` stores extracted prompt/message text when JSON extraction is available.
  - `.agents/state/hooks/subagents.jsonl` records observed `SubagentStart` and `SubagentStop` events.
- `last-<runtime>-<event>.json` files are convenience pointers only. They are intentionally overwritten and must not be used as the interview history.
- On-demand tool state and dependencies belong under `.agents/state/tools/<tool>` or the tool's own cache paths. They are not workflow state and must not be treated as active skills.
- Workflow state is stored under `.agents/state/workflows/<skill>/` when hooks can parse runtime JSON.
  - `deep-interview/<session-id>.json` tracks pending question gates, answers, and terminal workflow status.
  - Crystallized `deep-interview` states carry a `pipeline_lock` that blocks implementation mutation until `ralplan` owns or approves the handoff.
  - `deep-interview/events.jsonl` records gate, answer, state, and mutation-guard events.
  - `deep-interview/specs/deep-interview-<session-id>-<timestamp>.md` stores the crystallized final spec as a durable lock artifact.
  - `deep-interview/specs/index.jsonl` indexes persisted spec artifacts and sha256 values.
  - `ralplan/<session-id>.json` tracks review coverage, linked input spec sha256 values, pending plan approval, and approved handoff target.
  - `ralplan/events.jsonl` records review, plan state, approval, and mutation-guard events.
  - `ralplan/reviews/ralplan-review-<session-id>-<role>-r<round>-<timestamp>.md` stores planner, architect, and critic review passes.
  - `ralplan/reviews/index.jsonl` indexes persisted review artifacts and sha256 values.
  - `ralplan/plans/ralplan-<session-id>-<plan-id>-<timestamp>.md` stores the pending plan as a durable lock artifact with the linked deep-interview input sha256 when present.
  - `ralplan/plans/index.jsonl` indexes persisted plan artifacts, input spec sha256 values, and plan sha256 values.
  - `ultragoal/<session-id>.json` tracks runtime phase, active goal, source plan, and mutation-guard state for the hook.
  - `ultragoal/<session-id>/brief.md` stores the approved execution brief.
  - `ultragoal/<session-id>/goals.json` stores goal status, source metadata, evidence, and completion receipts.
  - `ultragoal/<session-id>/ledger.jsonl` records goal creation, start, checkpoint, and steering events.
- During active `deep-interview`, `ralplan`, or `ultragoal` goal-planning, the hook blocks obvious shell-based mutations and known write/edit tools unless `MEGARA_MUTATION_GUARD=warn` or `MEGARA_MUTATION_GUARD=off` is set.
- After `deep-interview` crystallizes, the hook still blocks obvious implementation mutation until a `ralplan` state becomes active or approved for the same session.
- `ultragoal` permits implementation mutation only after `.agents/bin/megara ultragoal complete-goals` selects an active goal.
- Codex App reads hooks at session start. After project-scope install, open a new saved-project or exact-directory session; projectless sessions may create a sibling directory without this harness.
- Codex `SessionStart` reinforces `caveman` so new or resumed sessions receive the default active style context.
- `deep-interview` and `ralplan` lock artifacts are hook-managed. Agents must not directly edit `.agents/state/workflows/deep-interview/**` or `.agents/state/workflows/ralplan/**`; direct write attempts are guarded even when a workflow is no longer active.
