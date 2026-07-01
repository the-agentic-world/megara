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
- If a crystallized `deep-interview` specification exists, treat that markdown artifact as the input lock. Reference it in the plan and do not contradict it without calling out the conflict.
- When an input lock exists, include the spec path or sha256 in the plan body so reviewers can trace the plan back to the crystallized requirement.
- The planner creates the first plan.
- The architect reviews system shape, boundaries, and tradeoffs.
- The critic rejects vague, unverifiable, or internally inconsistent plans.
- Iterate until the plan is executable, the user requests refinement, or a blocker is explicit.
- Do not finish with a pending-approval plan until planner, architect, and critic passes have all been recorded and the critic verdict is `OKAY`.
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

The pending-approval plan is allowed only after these review conditions are true:

- Latest planner pass is present.
- Latest architect pass is `CLEAR`, `WATCH`, or `OKAY`.
- Latest critic pass is `OKAY`.

After each planner, architect, or critic pass, append one review block. Do not write review notes to files directly; the hook records these blocks as durable review artifacts.

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

The final pending-approval response must contain the full markdown plan first. After the plan, append both parseable blocks exactly once. The hook records the markdown before these blocks as the locked plan artifact and computes `plan_sha256`.

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
