---
type: ComparativeAudit
title: Workflow UX Audit
description: Gajae-Code workflow behavior compared with Megara on Codex App.
timestamp: 2026-07-14
tags: [workflow, ux, codex, gajae-code, audit]
---

# Workflow UX Audit

## Scope

This audit compares Megara with Gajae-Code commit
[`774bc167`](https://github.com/Yeachan-Heo/gajae-code/commit/774bc1677190804017eda6ef8eef6654e40703cd).
The comparison uses Gajae-Code's bundled workflow skills, native `ask` tool,
workflow state runtime, and workflow-gate tests. It does not treat Gajae-Code's
TUI as a UI specification for Codex App.

Codex App owns the conversation UI and agent loop. Project hooks can add
developer context and enforce lifecycle checks, but they cannot create a native
question widget or silently enqueue another model turn. A `Stop` hook returning
`continue: false` stops the hook run; it is not a hidden follow-up-turn API.
`suppressOutput` is parsed but not implemented. See the
[Codex hooks documentation](https://developers.openai.com/codex/config-advanced#hooks).

## Actual Gajae-Code Flow

1. `deep-interview` resolves a threshold and topology before ordinary rounds.
2. Every answer is recorded, ambiguity is recomputed bidirectionally, and one
   next question is shown through the native `ask` tool.
3. Milestones trigger isolated lateral reviews before the next decision.
4. Crystallization writes a durable specification and opens an approval gate.
5. `ralplan` persists planner, architect, critic, revision, and final artifacts.
6. The native workflow broker validates approval answers and advances durable
   state. Its own session runtime can queue hidden follow-up messages.
7. `ultragoal` executes approved work against durable goals and evidence.

The strongest reusable idea is not the TUI. It is the lifecycle contract:
one visible decision, one durable state transition, one recoverable owner, and
no repeated approval for the same transition.

## Codex App Alternative

Megara uses Codex-native surfaces instead of imitating Gajae-Code's TUI:

- Assistant Markdown provides one question, numbered choices, and a recommendation.
- `UserPromptSubmit` binds the answer and injects hidden workflow context into
  the same user-selected turn.
- `Stop` validates and records output; it does not manufacture another turn.
- `SubagentStart` and `SubagentStop` provide review receipts.
- Project-local state and artifacts provide interruption recovery.
- Product-facing prose remains visible; hook instructions and state paths remain hidden.

## Iteration Checklist

Run this checklist after every workflow UX change. A row passes only when its
automated test and its Codex App payload simulation both pass.

| ID | Check | Required evidence | Result |
|---|---|---|---|
| W01 | Natural workflow entry needs no Plan mode or duplicate invocation. | App-surface `UserPromptSubmit` integration test. | Pass |
| W02 | One active question is visible with numbered choices and recommendation last. | Question parser and projection tests. | Pass |
| W03 | Every answer updates durable history and recomputes non-monotonic ambiguity. | Reassessment integration tests. | Pass |
| W04 | Milestone output contains an explicit crystallized requirement and concrete corrections. | Milestone parsing tests. | Pass |
| W05 | Selecting `ralplan` once starts planning in that same user-selected turn. | Transition integration test; no Stop continuation. | Pass |
| W06 | `ralplan` approval starts `ultragoal` or `team` without another approval. | Approval/handoff integration tests. | Pass |
| W07 | Missing reviews or artifacts block mutation without exposing hook prose. | Guard and metadata-leak tests. | Pass |
| W08 | Required subagents are observed, closed, and tied to the active revision. | Subagent receipt tests. | Pass |
| W09 | Resume is idempotent and does not duplicate a question or transition. | Repeated-event integration tests. | Pass |
| W10 | User-visible output follows locale and contains no runtime metadata. | Korean/App simulation and leak tests. | Pass |
| W11 | Direct, bounded work can bypass interview workflows. | Skill projection contract test. | Pass |
| W12 | Final execution reports product changes, verification, and actionable blockers only. | Ultragoal terminal tests. | Pass |

## Final Verification

The Codex App alternative passed the checklist without reproducing Gajae-Code's
native TUI:

1. A milestone response persists its visible crystallized sentence and two
   correction choices as a recoverable candidate. Confirmed interview answers,
   including the meaning of numeric selections, remain in that candidate.
2. Selecting `ralplan` once schedules the required lateral reviews through
   hidden hook context.
3. The final required review promotes the candidate, starts `ralplan`, and
   continues the same user-selected turn without `Stop` feedback.
4. Duplicate subagent events and later user prompts do not repeat the
   transition.
5. A projected Codex App payload consumes `deep-interview -> ralplan ->
   ultragoal`, ending with an active goal.

Verification commands:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Final result: all checklist rows passed, including 102 integration tests and 81
unit tests.

## Acceptance Rule

The workflow UX is ready only when W01-W12 all pass, `cargo fmt --check`,
`cargo clippy -- -D warnings`, and `cargo test` pass, and a projected Codex App
payload can consume `deep-interview -> ralplan -> ultragoal` without visible
hook feedback or repeated approval.

## Sources

- [Gajae-Code deep-interview skill](https://github.com/Yeachan-Heo/gajae-code/blob/774bc1677190804017eda6ef8eef6654e40703cd/packages/coding-agent/src/defaults/gjc/skills/deep-interview/SKILL.md)
- [Gajae-Code ralplan runtime](https://github.com/Yeachan-Heo/gajae-code/blob/774bc1677190804017eda6ef8eef6654e40703cd/packages/coding-agent/src/gjc-runtime/ralplan-runtime.ts)
- [Gajae-Code ask tool](https://github.com/Yeachan-Heo/gajae-code/blob/774bc1677190804017eda6ef8eef6654e40703cd/packages/coding-agent/src/tools/ask.ts)
- [Gajae-Code workflow approval tests](https://github.com/Yeachan-Heo/gajae-code/blob/774bc1677190804017eda6ef8eef6654e40703cd/packages/coding-agent/test/workflow-approval-gates.test.ts)
- [Codex hooks documentation](https://developers.openai.com/codex/config-advanced#hooks)
