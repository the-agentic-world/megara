---
name: ultragoal
description: Durable goal execution workflow with Megara checkpoints and verification receipts
argument-hint: "<approved goal or plan>"
handoff-policy: approved-execution-only
---

# Ultragoal

Use this workflow when an approved plan should be executed to completion with durable progress tracking.

## Contract

- Start from an approved `ralplan` handoff by default. Use direct execution only when the user request is already concrete enough and the direct run is explicitly recorded with `--allow-direct`.
- If `deep-interview` is active and not crystallized, do not advance into ultragoal.
- Keep one active goal at a time unless the work is explicitly split into independent lanes.
- Record durable state through the Megara CLI; do not rely only on chat memory.
- Resolve the CLI before running any command: `MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"`. Do not rely on bare `megara` being present on `PATH`.
- Record evidence before considering a goal complete.
- Treat missing tests, shallow evidence, failed review, or plan/code mismatch as blockers.
- Do not ask the user to resolve work the agent can investigate or fix.
- Write every user-facing sentence in the configured locale, including progress updates, active goal reports, verification notes, blocker reports, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.

## Runtime State

Use the project scope by default:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> <command>
```

The durable state lives under:

- `.agents/state/workflows/ultragoal/<session-id>/brief.md`
- `.agents/state/workflows/ultragoal/<session-id>/goals.json`
- `.agents/state/workflows/ultragoal/<session-id>/ledger.jsonl`

For global harnesses, set `MEGARA_BIN="${MEGARA_BIN:-$HOME/.megara/bin/megara}"` and use `--scope global`; the same files are stored under `~/.megara/state/workflows/ultragoal/<session-id>/`.

## Goal Creation

Create goals from the approved `ralplan` handoff for this session:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> create-goals
```

The command reads `.agents/state/workflows/ralplan/<session-id>.json`, verifies that the approved handoff target is `ultragoal`, verifies the approved plan sha256, and then creates goals from the locked plan artifact.

If the approved brief contains column-zero `@goal` markers, each marker starts a new goal:

```text
@goal: Board shell
Build the board UI and empty state.

@goal Scoring model
Implement scoring and verification.
```

If there are no `@goal` markers, Megara creates one goal from the whole brief.

Do not overwrite an existing session unless intentionally restarting:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> create-goals --force
```

For a concrete direct run without ralplan, make the bypass explicit:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> create-goals --allow-direct --brief "<concrete approved goal>"
```

## Execution Loop

1. Run `MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"; "$MEGARA_BIN" ultragoal --scope project --session-id <session-id> status`.
2. Run `MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"; "$MEGARA_BIN" ultragoal --scope project --session-id <session-id> complete-goals`.
3. Execute the returned active goal with the smallest correct change.
4. Run focused verification.
5. Run review and cleanup gates.
6. Record a checkpoint with exact evidence.
7. Continue until every goal is complete.

## Completion Gate

Complete checkpoints require `--quality-gate-json`.

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> checkpoint \
  --goal-id G001 \
  --status complete \
  --evidence "cargo test passed; changed src/game.rs and tests/game.rs" \
  --quality-gate-json /tmp/megara-quality-gate.json
```

The JSON must contain:

```json
{
  "architectReview": {
    "recommendation": "APPROVE",
    "architectureStatus": "CLEAR",
    "productStatus": "CLEAR",
    "codeStatus": "CLEAR",
    "evidence": "Review notes here.",
    "reviewedFiles": ["src/main.rs"],
    "blockers": []
  },
  "executorQa": {
    "status": "passed",
    "e2eStatus": "passed",
    "redTeamStatus": "passed",
    "evidence": "Test and manual verification notes here.",
    "commands": ["cargo test"],
    "artifactRefs": ["verification.log"],
    "blockers": []
  },
  "iteration": {
    "status": "passed",
    "fullRerun": true,
    "evidence": "Final rerun notes here.",
    "commands": ["cargo test"],
    "artifactRefs": ["verification.log"],
    "blockers": []
  }
}
```

Megara stores a completion receipt containing sha256 values for the evidence, quality gate, brief, and approved source plan when present. Referenced artifact files must exist and must not be empty.

## Blockers And Steering

For non-complete checkpoints, still record evidence:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> checkpoint \
  --goal-id G001 \
  --status blocked \
  --evidence "Blocked because the required API key is absent."
```

When the approved goal needs a controlled addition, add a pending subgoal:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> steer \
  --kind add-subgoal \
  --title "Add regression test" \
  --objective "Cover the discovered parser edge case before completion." \
  --evidence "The edge case was found while implementing G001."
```

For important context that should not create a goal:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> steer \
  --kind annotate-ledger \
  --evidence "Manual QA covered Safari and Chrome."
```

## Terminal State

When reporting workflow state to the runtime hook, include:

```text
Megara Workflow State:
- skill: ultragoal
- status: goal_planning|active|blocked|complete
- next: <next action>
```

The hook blocks product file mutation while status is `goal_planning`. After `"$MEGARA_BIN" ultragoal complete-goals` selects an active goal, report `status: active`; implementation edits are then expected and allowed.
