---
name: ralplan
description: Consensus planning workflow for implementation-ready plans
argument-hint: "[--interactive] [--deliberate] <task description>"
pipeline: [ralplan, ultragoal]
handoff-policy: approval-required
---

# Ralplan

Use this workflow after a request is clear enough to plan, but before implementation begins.

## Contract

- Ralplan is planning-only until execution is explicitly approved.
- Do not edit files, run mutating commands, commit, or push while Ralplan is active.
- If this `ralplan` run follows an approved `deep-interview` handoff, the current session must have a persisted crystallized markdown artifact. Treat only that artifact as the input lock.
- Do not use conversation-only `deep-interview` content as a substitute for the persisted lock after a `deep-interview` approval. If the current-session lock is missing, stale, or mismatched, stop with a blocker instead of producing a pending-approval plan.
- When a `deep-interview` input lock is required, include the current-session spec path and exact sha256 in the plan body and in the final `Megara Workflow State` block.
- `input_spec_sha256: none` is allowed only for direct `ralplan` runs that did not follow an approved `deep-interview` handoff.
- The planner creates the first plan.
- The architect reviews system shape, boundaries, and tradeoffs.
- The critic rejects vague, unverifiable, or internally inconsistent plans.
- Iterate until the plan is executable, the user requests refinement, or a blocker is explicit.
- Do not finish with a pending-approval plan until planner, architect, and critic passes have all been recorded, the planner and architect verdicts are approval-capable, and the critic verdict is `OKAY`.
- Always finish approved planning with a pending-approval plan, clear execution options, and the parseable gate blocks below.
- Write every user-facing sentence in the configured locale, including progress updates, plan headings, option labels, assumptions, risks, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.

## Planning Shape

The plan must include:

- Summary and intended outcome.
- In scope / out of scope.
- Affected files, modules, or runtime surfaces.
- Ordered tasks and dependencies.
- Acceptance criteria.
- Verification commands or evidence.
- Risks, tradeoffs, and rollback notes where relevant.

## Review Loop

Use this review order before producing the pending-approval plan:

1. Planner: draft the proportional execution plan from known facts.
2. Architect: review boundaries, affected surfaces, sequencing, and reversibility.
3. Critic: return `OKAY`, `ITERATE`, or `REJECT`.

If the critic returns `ITERATE`, revise once and run the critic pass again. If the critic still blocks, stop with the blocker instead of inventing certainty.

Planner `DRAFT` means the plan is still an intermediate draft. It may be recorded during review, but it is not approval-capable and must not be used in the final pending-approval response. Before asking for approval, the latest planner verdict must be `CLEAR`, `WATCH`, or `OKAY`.

The pending-approval plan is allowed only after these review conditions are true:

- Latest planner pass is `CLEAR`, `WATCH`, or `OKAY`.
- Latest architect pass is `CLEAR`, `WATCH`, or `OKAY`.
- Latest critic pass is `OKAY`.

After each planner, architect, or critic pass, append one review block. Do not write review notes to files directly; the hook records these blocks as durable review artifacts.

When producing the final pending-approval plan, include the latest planner, architect, and critic review blocks in the same final assistant message after the workflow state block. Progress/commentary messages are not durable hook input; if a review block appears only there, the plan will be rejected as `review_incomplete`.

```text
Megara Review Pass:
- role: planner|architect|critic
- round: 1
- verdict: DRAFT|CLEAR|WATCH|BLOCK|OKAY|ITERATE|REJECT
- summary: <configured-locale concise review summary>
- required_fixes:
  - <configured-locale required fix, or none>
```

Do not put this block inside code fences in the actual response.

## Plan Gate

The final pending-approval response must contain, in order:

1. The full markdown plan.
2. `Megara Plan Gate` exactly once.
3. `Megara Workflow State` exactly once.
4. Latest `Megara Review Pass` blocks for planner, architect, and critic.

The hook records the markdown before the plan/workflow gate blocks as the locked plan artifact and computes `plan_sha256`.

Use stable ids within the session:

```text
Megara Plan Gate:
- id: rp-<short-purpose>
- status: pending_approval
- question: <configured-locale approval question>
- options:
  - refine
  - approve_ultragoal
  - approve_team
  - stop_pending
- free_text: false

Megara Workflow State:
- skill: ralplan
- status: pending_approval
- plan_id: rp-<short-purpose>
- input_spec_sha256: <sha256 or none>
- next: approval
```

Do not put these blocks inside code fences in the actual response.

If a `deep-interview` handoff was approved but no matching persisted lock exists for the current session, do not emit `Megara Plan Gate`. End with:

```text
Megara Blocker Gate:
- workflow: ralplan
- status: blocked
- reason: persisted_deep_interview_lock_missing_or_mismatched
- implementation_allowed_now: false
```

Do not put this block inside code fences in the actual response.

## Approval Gate

Offer these terminal choices:

- Refine further.
- Approve execution via `ultragoal`.
- Approve execution via `team`.
- Stop with the plan pending approval.

When an external controller or user provides a parseable approval gate, it must bind to the exact locked plan:

```text
Megara Approval Gate:
- plan_id: rp-<short-purpose>
- plan_sha256: <64-char sha256>
- handoff_target: ultragoal|team
```

Do not put this block inside code fences in the actual approval response.
