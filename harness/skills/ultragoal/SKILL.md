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
- When the user selects `ultragoal` from the final `ralplan` choices, that selection is sufficient authorization. Immediately create and start goals from the approved plan; do not require a separate `$ultragoal` invocation or approval.
- Record evidence before considering a goal complete.
- Treat missing tests, shallow evidence, failed review, or plan/code mismatch as blockers.
- Do not ask the user to resolve work the agent can investigate or fix.
- Write every user-facing sentence in the configured locale, including progress updates, active goal reports, verification notes, blocker reports, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- For technical concepts, prefer natural terms in the configured locale when they exist. If an English-only technical term must remain, add a short explanation in the configured locale on first use, preferably as a footnote or parenthetical note.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.
- Run Megara CLI commands silently and use their output internally. Do not narrate session ids, state files, handoff files, goal-planning status, active-goal selection, checkpoint attempts, completion receipts, ledger writes, or quality-gate JSON handling in user-visible progress messages.
- Runtime artifact paths under `.megara/state`, `.megara/artifacts`, `~/.megara/state`, or `~/.megara/artifacts` are internal-only. Do not link them, cite them as deliverables, list them as changed files, or include them in final user-facing summaries. Translate their contents into product-facing verification notes instead.
- User-visible progress should mention only externally meaningful product work: the selected product issue, files being changed, verification being run, blockers the user can act on, and final results.
- Do not create user-visible progress messages such as "handoff file missing", "goal checkpoint recorded", "receipt created", "quality evidence updated", "active goal opened", "active goal selected", "approved goal opened", "open/select goal", "runtime state updated", "record evidence", "complete goal", "approved plan opened", "approved plan produced execution units", "instructions loaded", "승인된 목표 열림", "활성 목표 선택", "목표 열기", "목표 열고", "목표를 열고", "검증 증거 기록", "목표 완료 처리", "ultragoal 승인", "승인 계획", "실행 단위 생성", or "지침 읽고". These are runtime internals.
- After `start-goal`, if you send progress at all, describe the product work being started, not the Megara goal state. Use product-facing phrasing such as "2048 상태 UI 개선을 시작합니다" or "2048 검증을 시작합니다. 기준선 테스트와 evidence 보존을 확인합니다." Do not mention goals being opened, selected, approved, converted, or split into execution units.

## CLI Usage

Use the project scope by default:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> <command>
```

For global harnesses, set `MEGARA_BIN="${MEGARA_BIN:-$HOME/.megara/bin/megara}"` and use `--scope global`.

## Verification Evidence

Use product-facing verification notes as the checkpoint `--evidence` value. Do not create, edit, copy, link, or list Megara runtime files yourself; runtime files are owned by Megara hooks and CLI commands.

Quality gate JSON may be passed inline to `--quality-gate-json`. `artifactRefs` is optional. If you include `artifactRefs`, use only existing product-facing files that are already part of the approved work or verification output; never create runtime evidence files just to satisfy the gate.

## Goal Creation

Create goals from the approved `ralplan` handoff for this session:

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> create-goals
```

The command verifies the approved handoff and creates goals from the locked plan artifact. Use the command output internally only.

If the approved brief contains column-zero `@goal` markers, each marker starts a new goal:

```text
@goal: Board shell
Build the board UI and empty state.

@goal Scoring model
Implement scoring and verification.
```

If there are no `@goal` markers, Megara first tries to split the approved plan's `Steps` / `작업 순서` section into one goal per top-level numbered item. If there is no usable steps section, Megara creates one goal from the whole brief.

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
2. Run `MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"; "$MEGARA_BIN" ultragoal --scope project --session-id <session-id> start-goal`.
3. Execute the returned active goal with the smallest correct change.
4. Run focused verification.
5. Run review and cleanup gates.
6. Record a checkpoint with exact evidence.
7. Continue until every goal is complete.

## Completion Gate

Complete checkpoints require `--quality-gate-json`. Prefer inline JSON so no temporary project file is created solely for the gate.

```bash
MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}"
"$MEGARA_BIN" ultragoal --scope project --session-id <session-id> checkpoint \
  --goal-id G001 \
  --status complete \
  --evidence "cargo test passed; changed src/game.rs and tests/game.rs" \
  --quality-gate-json '{"architectReview":{"recommendation":"APPROVE","architectureStatus":"CLEAR","productStatus":"CLEAR","codeStatus":"CLEAR","evidence":"Architecture, product behavior, and code boundaries reviewed.","reviewedFiles":["src/game.rs","tests/game.rs"],"blockers":[]},"executorQa":{"status":"passed","e2eStatus":"passed","redTeamStatus":"passed","evidence":"Focused tests and manual regression checks passed.","commands":["cargo test"],"blockers":[]},"iteration":{"status":"passed","fullRerun":true,"evidence":"Final verification reran after cleanup.","commands":["cargo test"],"blockers":[]}}'
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
    "blockers": []
  },
  "iteration": {
    "status": "passed",
    "fullRerun": true,
    "evidence": "Final rerun notes here.",
    "commands": ["cargo test"],
    "blockers": []
  }
}
```

`executorQa.e2eStatus` may be `passed` or `skipped`. Use `skipped` only when the change has no UI or end-to-end behavioral surface, and put the reason in `executorQa.evidence`.

Megara stores a completion receipt containing sha256 values for the evidence, quality gate, brief, and approved source plan when present. If optional referenced product files are supplied, they must exist and must not be empty.

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

When reporting progress, output only user-facing prose. Do not emit `Megara Workflow State`, HTML comments, YAML-like control blocks, JSON, code fences, raw runtime metadata, or `.megara` runtime artifact links. The visible response should summarize product progress, user-actionable blockers, or completion in user-friendly language.

Runtime state is managed by the `megara ultragoal` CLI commands. The hook blocks product file mutation while status is `goal_planning`. After `"$MEGARA_BIN" ultragoal start-goal` selects an active goal, implementation edits are expected and allowed because the CLI has already updated runtime state.
