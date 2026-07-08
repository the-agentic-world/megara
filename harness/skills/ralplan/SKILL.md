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
- When a `deep-interview` input lock is required, use the current-session spec path and exact sha256 only through runtime state. Do not show `spec_path`, `spec_sha256`, or `input_spec_sha256` in visible plan prose.
- `input_spec_sha256: none` is allowed only for direct `ralplan` runs that did not follow an approved `deep-interview` handoff.
- The planner creates the first plan.
- The architect reviews system shape, boundaries, and tradeoffs.
- The critic rejects vague, unverifiable, or internally inconsistent plans.
- In the Codex runtime adapter, hooks inject a hidden requirement to run planner, architect, and critic as context-only, tool-free subagents. The pending-approval plan is continued until those subagent receipts are observed.
- Iterate until the plan is executable, the user requests refinement, or a blocker is explicit.
- Do not finish with a pending-approval plan until planner, architect, and critic passes have all been recorded, the planner and architect verdicts are approval-capable, and the critic verdict is `OKAY`.
- Always finish approved planning with a pending-approval plan and clear execution options. Do not output runtime metadata.
- Write every user-facing sentence in the configured locale, including progress updates, plan headings, option labels, assumptions, risks, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- For technical concepts, prefer natural terms in the configured locale when they exist. If an English-only technical term must remain, add a short explanation in the configured locale on first use, preferably as a footnote or parenthetical note.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.
- Do not send progress messages that merely narrate internal workflow mechanics such as reading this skill, checking locks, running review roles, inferring review coverage, persisting plans, or preparing handoff state. Keep those details internal. User-visible output should be the plan, a blocker, or an approval question.
- Do not send commentary while spawning or waiting for planner, architect, or critic subagents. Wait silently and then send only the final pending-approval plan, a user-friendly blocker, or the approval question.

## Planning Shape

The plan must include:

- Summary and intended outcome.
- In scope / out of scope.
- Affected files, modules, or runtime surfaces.
- Ordered tasks and dependencies.
- Acceptance criteria.
- Verification commands or evidence.
- Risks, tradeoffs, and rollback notes where relevant.
- Baseline failure handling whenever verification commands are part of the
  plan: if a verification command already fails before the change, classify it
  as pre-existing, avoid expanding scope to fix unrelated failures, and verify
  no new failures plus targeted evidence for the approved work.
- Plan-owned clarification whenever the missing detail can be closed by a
  conservative verification rule. Do not block on details such as which exact
  status widgets, keyboard no-op behavior, accessibility labels, baseline-test
  semantics, or report surfaces to inspect. Pick the stricter product-facing
  criterion, state it in the plan, and let execution validate it.

The plan body must stay product-facing. Do not put workflow or handoff names
such as `ralplan`, `ultragoal`, or `team` inside the plan body. Reserve approval
targets for the final numbered approval choices only.

## Review Loop

Before producing the pending-approval plan, create one concrete internal draft
plan and get three isolated Codex subagent reviews: planner, architect, and
critic. Every review prompt must include the draft plan under review, not only
the raw task or spec.

The planner checks proportionality and execution order. The architect checks
boundaries, affected surfaces, sequencing, and reversibility. The critic returns
`OKAY`, `ITERATE`, or `REJECT`.

Subagent prompts must be short and must forbid tools, file reads, file writes,
Megara workflow/skill invocation, nested subagents, implementation, and progress
output. The main session owns the final user-facing plan and approval question.

If the critic returns `ITERATE`, revise once and run the critic pass again. Turn
critic requests about verification detail into explicit plan criteria instead of
asking the user to re-decide them. If the critic still blocks, stop only when the
missing fact cannot be safely planned around. If the blocker is user-resolvable,
ask one compact clarification question with numbered choices; do not end with a
generic list of unresolved review notes.

If any subagent says no plan was provided, that pass is invalid. Rerun only that
role with the draft plan included instead of stopping with a no-plan blocker.

Planner `DRAFT` means the plan is still an intermediate draft. It may be recorded during review, but it is not approval-capable and must not be used in the final pending-approval response. Before asking for approval, the latest planner verdict must be `CLEAR`, `WATCH`, or `OKAY`.

The pending-approval plan is allowed only after these review conditions are true:

- Latest planner pass is `CLEAR`, `WATCH`, or `OKAY`.
- Latest architect pass is `CLEAR`, `WATCH`, or `OKAY`.
- Latest critic pass is `OKAY`.

After each planner, architect, or critic pass, keep the review result internal. Do not write review notes to files directly and do not output review metadata, HTML comments, YAML-like control blocks, JSON, or code fences in the main user-facing response. Runtime hooks record subagent receipts and infer review coverage from the visible pending-approval plan.

## Plan Gate

The final pending-approval response must contain visible prose only, in order:

1. The full markdown plan.
2. A concise visible approval question with numbered choices.
3. Four numbered choices: refine, approve via `ultragoal`, approve via `team`, or leave pending.

The hook records the visible markdown plan as the locked plan artifact and computes `plan_sha256`. Do not print `Megara Plan Gate`, `Megara Workflow State`, `Megara Review Pass`, `input_spec_sha256`, `plan_sha256`, `spec_path`, or any other runtime metadata. Runtime state is managed only by hooks.

If a `deep-interview` handoff was approved but no matching persisted lock exists for the current session, do not produce a pending-approval plan. Explain the blocker in user-friendly language without raw gate labels, and do not output metadata.

If the previous crystallized `deep-interview` spec explicitly says the next assistant turn should start `ralplan`, begin planning from the locked spec without asking another transition question. Still keep `ralplan` planning-only until the user approves one of the final execution choices.

## Approval Gate

Offer these terminal choices:

- Refine further.
- Approve execution via `ultragoal`.
- Approve execution via `team`.
- Stop with the plan pending approval.

Normal user approval should be a number or natural-language choice. Do not output `Megara Approval Gate` or any parseable approval metadata. Runtime hooks bind the user decision to the current locked plan in state.
