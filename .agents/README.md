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

## Agents

- `executor`
- `planner`
- `architect`
- `critic`

## Runtime Hooks

- `hooks/megara-hook.sh`: portable hook runner used by runtime adapters to keep lightweight event state without breaking the agent runtime.
- Hook state is append-only by default:
  - `.agents/state/hooks/events.jsonl` indexes every hook event.
  - `.agents/state/hooks/payloads/<runtime>/<event>/*.json` stores every raw payload.
  - `.agents/state/hooks/conversation-events.jsonl` indexes user prompts and assistant stop messages.
  - `.agents/state/hooks/conversation.jsonl` stores extracted prompt/message text when JSON extraction is available.
- `last-<runtime>-<event>.json` files are convenience pointers only. They are intentionally overwritten and must not be used as the interview history.
- Workflow state is stored under `.agents/state/workflows/<skill>/` when hooks can parse runtime JSON.
  - `deep-interview/<session-id>.json` tracks pending question gates, answers, and terminal workflow status.
  - `deep-interview/events.jsonl` records gate, answer, state, and mutation-guard events.
  - `deep-interview/specs/deep-interview-<session-id>-<timestamp>.md` stores the crystallized final spec as a durable lock artifact.
  - `deep-interview/specs/index.jsonl` indexes persisted spec artifacts and sha256 values.
- During active `deep-interview`, the hook blocks obvious Bash-based file mutations unless `MEGARA_MUTATION_GUARD=warn` or `MEGARA_MUTATION_GUARD=off` is set.
