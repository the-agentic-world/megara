---
name: deep-interview
description: Socratic requirements interview with percentage ambiguity scoring before execution approval
argument-hint: "[--quick|--standard|--deep] <idea or vague description>"
pipeline: [deep-interview, ralplan]
handoff-policy: approval-required
---

# Deep Interview

Deep Interview is a Socratic requirements workflow. It turns a vague request into a specification by exposing assumptions, scoring remaining ambiguity as a percentage, and refusing to move into planning or execution until the work is clear enough.

## Contract

- Do not implement during deep-interview.
- Ask one question at a time.
- Write every user-facing sentence in the configured locale, including progress updates, questions, option labels, assumptions, score explanations, and final summaries.
- Keep parseable block keys, file paths, commands, config keys, API names, and quoted source text unchanged.
- In parseable blocks, free-text values such as `question`, `options`, `rationale`, and `summary` should use the configured locale unless they are technical literals.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.
- Inspect repository facts before asking the user about facts the repository can answer.
- Start with Round 0 topology confirmation: identify top-level components or outcomes and ask whether the shape is correct.
- Score remaining ambiguity after each answer as a percentage, not a 0-10 rating.
- Target the weakest active component and dimension each round.
- Continue until ambiguity is at or below the resolved threshold, or the user explicitly exits early.
- End with a pending-approval specification and a recommendation to continue through `ralplan`.
- Recording interview state is allowed and required; it is not implementation work.

## Use When

- The request is broad, vague, high-risk, or likely to hide assumptions.
- The user asks to be interviewed, challenged, or clarified before implementation.
- A wrong assumption would waste meaningful work.
- The task spans multiple components, workflows, actors, or acceptance criteria.

Do not use this when the request already has concrete files, behavior, constraints, and verification criteria. In that case, execute directly or use `ralplan` if only planning is requested.

## Phase 0: Resolve Threshold

Resolve the ambiguity threshold before the first question.

- Default threshold: `15% remaining ambiguity`.
- If the user explicitly gives a stricter or looser threshold, use that and name it.
- If project or runtime settings expose a deep-interview threshold, use that and name the source.

Before Round 0, report:

```text
Deep Interview threshold: NN% remaining ambiguity (source: default|user|project|runtime)
```

## Ambiguity Scoring

Report ambiguity as remaining uncertainty from `0%` to `100%`.

- `0%`: fully specified, no meaningful planning assumption remains.
- `15%`: low ambiguity, minor assumptions only.
- `35%`: moderate ambiguity, at least one material choice remains.
- `60%`: high ambiguity, implementation shape can still change.
- `85%`: severe ambiguity, planning would mostly guess.
- `100%`: unusable for planning.

Score clarity per active topology component. Then derive dimension scores from the weakest or coverage-weighted active component scores. Deferred components are excluded from ambiguity math but must remain visible in the final spec.

Use these weighted dimensions:

| Dimension | Greenfield Weight | Brownfield Weight | What It Measures |
|-----------|-------------------|-------------------|------------------|
| Outcome clarity | 20 | 18 | The end state and core user-visible outcome are unambiguous. |
| Scope boundary | 18 | 15 | In-scope, out-of-scope, and deferrals are explicit. |
| User value | 12 | 10 | The primary user and reason this matters are known. |
| Technical constraints | 16 | 16 | Runtime, data, integration, migration, and operational limits are clear. |
| Verification | 22 | 21 | Acceptance criteria can be tested or reviewed. |
| Risk/context | 12 | 20 | Risks are known; brownfield changes are grounded in repo evidence. |

Calculate weighted clarity as `sum(dimension_clarity_percent * weight) / 100`.
Calculate ambiguity as `100 - weighted_clarity`.

After each user answer, report at minimum:

```text
Ambiguity: NN% remaining
Weakest dimension: <dimension> (MM%)
Next target: <component> / <dimension> — <one-sentence rationale>
```

Ambiguity is bidirectional and non-monotonic. Later answers can increase ambiguity when they contradict established facts, add scope, expose internal inconsistency, or fail to answer the targeted gap. Surface the rise through the normal score report and target the next question at the affected component/dimension.

Do not stop the interview until:

- overall ambiguity is at or below the resolved threshold,
- no dimension has more than `25%` remaining ambiguity,
- acceptance criteria and verification are concrete enough for `ralplan`.

## Round 0: Topology Confirmation

Before scoring, enumerate top-level components from the user's idea and any repo context.

- Prefer 1-6 components.
- Components are outcomes that can succeed or fail independently.
- Do not collapse sibling components just because one component is described in more detail.
- Allow user-confirmed deferrals.

Ask exactly one Round 0 confirmation question:

```text
Round 0 | Topology confirmation | Ambiguity: not scored yet

I'm reading this as N top-level component(s):
1. <component>: <one-sentence outcome>
2. ...

Is that topology right? Should anything be added, removed, merged, split, or deferred?
```

After the answer, carry this topology forward for every score, progress report, and final spec.

## Phase 1: Context Setup

Classify the interview as greenfield or brownfield.

- Greenfield: no meaningful existing implementation or the user is asking for a new standalone artifact.
- Brownfield: existing source, config, data, or behavior will be changed or integrated.

For brownfield work, gather facts before asking decision questions:

- Search/read focused files, docs, package manifests, tests, or configuration.
- Cite the file path, symbol, or pattern that triggered the question.
- Do not ask the user to restate facts the repository can answer.

If the initial context is too large, summarize it first. Preserve user intent, constraints, decisions, unknowns, cited files, and explicit non-goals. Use the summary for scoring and questions instead of carrying raw oversized text forward.

## Interview Depth

Cover these checkpoints before producing the pending-approval specification. Continue asking one question at a time until each active topology component has enough coverage.

- topology and intended outcome,
- user value and primary user,
- in-scope and out-of-scope boundaries,
- runtime, data, integration, migration, and operational constraints,
- acceptance criteria and test or review method,
- risks, rollback, security, privacy, or destructive actions.

If a user says "you decide", "anything is fine", or gives an uncertain answer, make one conservative assumption, lower only the dimensions that the assumption actually resolves, and ask the next highest-value question.

## Phase 2: Interview Loop

For each round:

1. Pick the active component/dimension pair with the weakest clarity.
2. Rotate between similarly weak components so one detailed area does not hide sibling ambiguity.
3. State why that component/dimension is the bottleneck.
4. Ask one targeted question.
5. Refine any substantial free-text answer into a compact interpretation and confirm that nothing was lost.
6. Score all dimensions and active components.
7. Update established facts, disputed facts, deferrals, and open gaps.
8. Record the round locally.

Every user-facing question must include a parseable gate block immediately after the question prose. Use this exact shape and leave a blank line after the block:

```text
Megara Question Gate:
- id: di-r<N>-<short-slug>
- round: <N>
- component: <component-slug>
- dimension: <Outcome clarity|Scope boundary|User value|Technical constraints|Verification|Risk/context>
- question: <single question text>
- options:
  - <option 1>
  - <option 2>
- free_text: true
```

Rules:

- `id` must be stable for the question and unique within the interview.
- `options` may be empty only when the question requires free text.
- `free_text` must be `true` when the user may answer outside the listed options.
- Do not put implementation instructions inside the gate.
- Do not ask another question outside the gate in the same assistant turn.

Question styles:

| Dimension | Use This Style |
|-----------|----------------|
| Outcome clarity | Ask what exactly happens when the work succeeds. |
| Scope boundary | Ask what is included, excluded, deferred, or intentionally unsupported. |
| User value | Ask who benefits, why now, and what pain disappears. |
| Technical constraints | Ask about runtime, data, integration, compatibility, operational, or migration boundaries. |
| Verification | Ask what test, review, demo, or observable behavior proves completion. |
| Risk/context | Ask about destructive actions, privacy/security, rollback, brownfield fit, or cited repo evidence. |

When the concept itself keeps shifting, ask an ontology question: what is the core entity, and which named things are supporting views, states, or containers?

## Established Facts And Trigger Handling

Promote stable answers into established facts with source round evidence. Do not delete contradicted facts; mark them disputed.

Ambiguity-raising triggers:

- Direct contradiction: an answer contradicts an established fact.
- Internal inconsistency: two requirements cannot both hold.
- Low-quality answer: the response avoids the targeted gap.
- Scope expansion: the response adds a component, entity, integration, constraint, or deliverable.

When a trigger occurs:

- lower the affected component/dimension clarity,
- let the weighted formula raise ambiguity,
- report the trigger in the progress summary,
- target the next question at the affected gap.

## Milestones And Lateral Review

Use milestone bands:

| Band | Ambiguity |
|------|-----------|
| initial | >60% |
| progress | 60%-31% |
| refined | above threshold through 30% |
| ready | <= threshold |

At milestone transitions, briefly run an internal lateral review before the next question. Use the existing internal fragments when available:

- `researcher`: external facts, prior art, version/compatibility constraints.
- `contrarian`: assumptions that may be false or habitual.
- `simplifier`: the smallest valuable version.
- `architect`: system shape, ownership, integration, and migration risks.

Fold only the highest-value finding into the next single question. The panel does not add extra questions, does not decide requirements, and does not permit implementation.

## Closure Gates

Passing the numeric threshold is not enough.

Before writing the final spec:

1. Closure audit: confirm every active component has outcome, scope, constraints, verification, and risk/context coverage. If a material gap remains, explain the gap and ask one more targeted question.
2. Restate gate: collapse the intended outcome into one sentence and ask the user to confirm whether that sentence alone would lead to the desired result.

Only crystallize the spec after both gates pass or the user explicitly exits early with known ambiguity.

When the final pending-approval spec is crystallized, include this parseable state block at the end:

```text
Megara Workflow State:
- skill: deep-interview
- status: crystallized
- ambiguity: <NN%>
- next: ralplan
```

The final pending-approval spec and the `Megara Workflow State` block must be in the same assistant response. Runtime hooks persist that full response as the locked markdown artifact for the interview. A standalone workflow-state block without the final spec body is not a valid crystallized handoff and should not be used except to diagnose a missing-artifact failure.

If the user explicitly cancels the interview, use `status: cancelled`. If the interview is still active and asking more questions, do not emit this workflow state block.

## Local Record

Runtime hooks should persist raw prompts and assistant messages locally under `.agents/state/hooks/`.

- `events.jsonl`: append-only hook event index.
- `payloads/<runtime>/<event>/*.json`: append-only raw payload snapshots.
- `last-<runtime>-<event>.json`: convenience pointer to the latest payload only.
- `conversation-events.jsonl`: chronological user/assistant event index.
- `conversation.jsonl`: extracted user prompt and assistant message text when the hook runtime can parse JSON.

When a crystallized final response includes `Megara Workflow State:`, runtime hooks should also persist the full final response as a markdown lock artifact:

- `.agents/state/workflows/deep-interview/specs/deep-interview-<session-id>-<timestamp>.md`
- `.agents/state/workflows/deep-interview/specs/index.jsonl`

The matching session JSON should reference `spec_path`, `spec_sha256`, and `spec_persisted_at`.

Do not treat `last-*` files as durable interview history. If a semantic interview ledger is needed in your response, summarize from the conversation history and the append-only hook logs, not from `last-*`.

At the end of every scored round, include a compact ledger update in the assistant message so the append-only hook log captures the semantic Q&A:

```text
Interview ledger update:
- Round: N
- Component: <component>
- Question: <question summary>
- Answer: <confirmed answer summary>
- Ambiguity: <previous>% -> <current>%
- Established facts: <added/disputed facts or none>
- Open gaps: <remaining gaps>
```

## Output

Produce a pending-approval specification:

- Metadata: threshold, source, rounds, final ambiguity, greenfield/brownfield, status.
- Clarity breakdown: dimensions, weights, weighted scores, final ambiguity.
- Topology: every active and deferred component.
- Established facts and disputed facts.
- Trigger history: contradictions, inconsistencies, evasive answers, and scope expansions.
- Goal: one confirmed sentence plus detail.
- In scope / out of scope / deferrals.
- Constraints and risks.
- Acceptance criteria.
- Technical context: repo evidence for brownfield work; chosen constraints for greenfield work.
- Ontology: key entities, attributes, and relationships when applicable.
- Interview transcript summary with all rounds.
- Suggested next workflow: normally `ralplan`.

End in pending approval and include the `Megara Workflow State` block in the same response so the runtime can persist the markdown spec artifact. Do not start implementation from this workflow.
