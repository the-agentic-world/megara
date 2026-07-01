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
- The planner creates the first plan.
- The architect reviews system shape, boundaries, and tradeoffs.
- The critic rejects vague, unverifiable, or internally inconsistent plans.
- Iterate until the plan is executable or a blocker is explicit.
- Always finish with a pending-approval plan and clear execution options.
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

## Approval Gate

Offer these terminal choices:

- Refine further.
- Approve execution via `ultragoal`.
- Approve execution via `team`.
- Stop with the plan pending approval.
