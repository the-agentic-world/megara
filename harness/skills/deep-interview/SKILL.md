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
- Do not copy English section headings into user-facing output. Translate final-spec labels such as `Round 0: Topology Confirmation`, `remaining ambiguity`, `weakest dimension`, and `next target` into the configured locale when they appear in the final artifact.
- Inspect repository facts before asking the user about facts the repository can answer.
- Start with Round 0 topology confirmation: identify top-level components or outcomes and ask whether the shape is correct.
- Score remaining ambiguity after each answer as a percentage, not a 0-10 rating.
- Keep active interview output compact for humans; do not include technical hook blocks in active user-facing answers.
- Show the current ambiguity score on every active interview question.
- Use exactly four visible options on every active interview question: three concrete choices plus one configured-locale free-text catch-all option for answers outside the listed choices.
- Target the weakest active component and dimension each round.
- Continue until ambiguity reaches the active ambiguity target, then ask whether to crystallize for `ralplan` or continue to the next stricter target.
- End with a pending-approval specification and a configured-locale next-step suggestion to continue through `ralplan`.
- Recording interview state is allowed and required; it is not implementation work.
- Runtime hooks own `.megara/state/workflows/deep-interview/**` and `.megara/state/workflows/ralplan/**`. Do not inspect, edit, repair, or synthesize those files to force a handoff.
- When the final crystallized spec is emitted, make it the final response of that assistant turn. Do not start `ralplan`, run tools, inspect state, or continue the workflow in the same turn.

## Use When

- The request is broad, vague, high-risk, or likely to hide assumptions.
- The user asks to be interviewed, challenged, or clarified before implementation.
- A wrong assumption would waste meaningful work.
- The task spans multiple components, workflows, actors, or acceptance criteria.

Do not use this when the request already has concrete files, behavior, constraints, and verification criteria. In that case, execute directly or use `ralplan` if only planning is requested.

## Phase 0: Resolve Threshold

Resolve the ambiguity threshold before the first question.

- Default ladder: `15% -> 5% -> 2% -> 0% remaining ambiguity`.
- Start with `15%` as the active ambiguity target.
- If the user explicitly gives a stricter or looser threshold, use that as an explicit override and name it.
- If project or runtime settings expose a deep-interview threshold, use that and name the source.

Before the first topology question, resolve the active target internally. Do not print a separate threshold line in active question output. Include the final target and source in the final crystallized specification.

```text
<configured-locale threshold label>: NN% <configured-locale remaining ambiguity> (source: default|user|project|runtime)
```

## Ambiguity Target Ladder

Use the default ambiguity target ladder unless the user or project configuration explicitly overrides it:

```text
15% -> 5% -> 2% -> 0%
```

Each target means the interview has enough clarity for that precision level only after the closure gates also pass.

- At `15%`, stop asking ordinary interview questions and ask whether to crystallize for `ralplan` now or continue deep-interview to `5%`.
- At `5%`, stop asking ordinary interview questions and ask whether to crystallize for `ralplan` now or continue deep-interview to `2%`.
- At `2%`, stop asking ordinary interview questions and ask whether to crystallize for `ralplan` now or continue deep-interview to `0%`.
- At `0%`, do not ask another milestone decision. Crystallize immediately for `ralplan` after closure gates pass, and show `0%` as the final visible ambiguity score in the crystallized spec.

Milestone decision questions are still active interview questions. They must show the current ambiguity score and exactly four visible numbered options:

1. Proceed to `ralplan` with the current crystallized spec.
2. Continue deep-interview to the next ambiguity target.
3. Continue deep-interview only on a named component or risk.
4. Direct input / not in the listed options.

If the user chooses option 2, lower the active target to the next ladder step and continue interviewing the weakest active gap. If the user chooses option 3, keep the next stricter target unless the user explicitly names a different target. If the user chooses option 1, write the final pending-approval spec as the final response of that assistant turn.

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

After each user answer, update the scores and carry them into the final crystallized specification. Every active question turn must show exactly one compact ambiguity score line before the question:

```text
<configured-locale ambiguity label>: NN%
```

For Round 0, estimate the initial ambiguity from the user's request and repository facts. If there is not enough information to make a useful estimate, show `100%`.

Do not show dimension-level score details in active question turns. If you need a private score note, keep it out of the user-facing answer.

Ambiguity is bidirectional and non-monotonic. Later answers can increase ambiguity when they contradict established facts, add scope, expose internal inconsistency, or fail to answer the targeted gap. Reflect the change in internal scoring and target the next question at the affected component/dimension.

Do not stop ordinary interview questions until:

- overall ambiguity is at or below the active ambiguity target,
- no dimension has more than `25%` remaining ambiguity,
- acceptance criteria and verification are concrete enough for `ralplan`.

At `15%`, `5%`, and `2%`, reaching the active target opens the milestone decision step; it does not automatically crystallize. At `0%`, reaching the active target crystallizes immediately after closure gates pass.

When the active target is `0%`, do not crystallize at `1%` or any other non-zero score. If final restatement confirmation removes the last meaningful planning assumption, explicitly set the final score to `0%` and include that score in the final spec. If it does not remove the last assumption, ask one more compact targeted question instead of finalizing.

## Runtime-Backed Multi-Turn Contract

Megara hooks back this workflow with append-only local state. Treat active questions as main-session UX, not as hidden metadata protocol.

- Codex App delegation wrappers such as `<codex_delegation><input>...</input>` are runtime transport details. The hook records the effective user prompt from inside `<input>`; do not mirror wrappers in user-facing prose.
- Each user answer should respond only to the current visible question. The runtime records the answer, pending question, and conversation event locally.
- Use read-only subagents for lateral review at milestone transitions and before final crystallization when the runtime exposes subagent tools. Use them for research, contradiction checks, simplification, or architecture blind-spot checks.
- Do not move the active question/answer loop into a subagent. The user-facing question stays in the main session.
- If subagent tools are unavailable, continue in the main session and keep the same read-only review discipline.
- Once the final spec crystallizes, stop. The next workflow must be `ralplan`, and implementation mutation is blocked by the runtime until `ralplan` owns or approves the handoff.

## Codex Plan-Mode Activation

When running under the Codex runtime adapter, native Codex Plan mode is required for the first deep-interview turn.

Runtime hooks attempt to activate Codex Plan mode before Round 0 when all conditions are true:

- current request starts deep-interview,
- current request did not already start with `/plan`,
- hook payload does not report `permission_mode: plan`.

If automatic activation succeeds, begin Round 0 normally.

If automatic activation fails, do not begin Round 0. Tell the user in the configured locale to activate Codex Plan mode first, then resend the same deep-interview request. Do not offer a "continue without Plan mode" option.

Do not show an ambiguity score, record a pending interview question, emit `Megara Workflow State`, inspect files, run tools, or start implementation during Plan-mode activation handling. A `/plan` text prefix is not enough by itself; skip this section only when the runtime or transcript reports Plan mode.

## Round 0: Topology Confirmation

Before scoring, enumerate top-level components from the user's idea and any repo context.

- Prefer 1-6 components.
- Components are outcomes that can succeed or fail independently.
- Do not collapse sibling components just because one component is described in more detail.
- Allow user-confirmed deferrals.

Ask exactly one first-round topology confirmation question. Use configured-locale prose for all visible text:

```text
<configured-locale ambiguity label>: NN%

<configured-locale sentence explaining that the request is being read as N components>:
1. <component>: <one-sentence outcome>
2. ...

<configured-locale single confirmation question about adding, removing, merging, splitting, or deferring components>

1. <configured-locale accept as-is option>
2. <configured-locale adjust components option>
3. <configured-locale defer or prioritize components option>
4. <configured-locale direct input / not in listed options>
```

After the answer, carry this topology forward for internal scoring and the final spec.

## Compact Visible Output

Active interview turns must be compact. Show only the information needed for the user to answer the next question:

1. Exactly one compact ambiguity score line.
2. One short context sentence only when it materially helps the user answer.
3. One targeted question.
4. A short numbered option list with exactly four options.

Do not print technical hook-gate headers, parseable gate blocks, full score tables, dimension score tables, full topology tables, all established facts, all open gaps, trigger history, lateral-review notes, transcript summaries, semantic ledger updates, or internal reasoning during active question turns. Keep those details in local records and the final crystallized lock artifact.

Never include labels such as `weakest dimension`, `next target`, `Interview ledger update`, `Established facts`, or `Open gaps` in an active question turn. The user only needs the ambiguity score, next question, and answer choices.

Final crystallized output may be longer because it becomes the persisted markdown lock artifact. Even then, avoid duplicating round details beyond what is needed for `ralplan`.

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

Every user-facing question must show a short numbered visible option list, so the user can answer by number. The list must contain exactly three concrete choices followed by one configured-locale free-text catch-all.

Do not include technical gate blocks in active question answers. Runtime hooks infer the pending question from the last visible question line and the following visible numbered options. Use this visible shape:

```text
<configured-locale ambiguity label>: NN%

<single question text?>

1. <option 1>
2. <option 2>
3. <option 3>
4. <configured-locale direct input / not in listed options>
```

Rules:

- The visible question should be one line ending with a question mark.
- Do not omit `options`; provide exactly four visible options.
- Number each option from 1 in order.
- The user may answer with the option number or with free text.
- Options 1-3 must be concrete choices that answer the question.
- Option 4 must always be a configured-locale catch-all such as "direct input / not in listed options". This option is visible UX, not a restriction on free-text answers.
- Do not put implementation instructions in the visible options.
- Legacy parseable gate blocks are supported by runtime hooks for backward compatibility only; do not emit them in new active question answers.
- Do not ask another question in the same assistant turn.

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
| refined | above active target through 30% |
| target reached | <= active target |

At milestone transitions, run a read-only lateral review before the next question. Use the existing internal fragments when available:

- `researcher`: external facts, prior art, version/compatibility constraints.
- `contrarian`: assumptions that may be false or habitual.
- `simplifier`: the smallest valuable version.
- `architect`: system shape, ownership, integration, and migration risks.

When the runtime supports subagent tools, request exactly one read-only subagent reviewer for each milestone transition. Choose the persona that matches the weakest remaining ambiguity dimension:

- `researcher` for missing external facts, versions, compatibility, or prior art.
- `contrarian` for contradictions, hidden assumptions, or risky defaults.
- `simplifier` for oversized scope or unclear minimum viable value.
- `architect` for ownership, integration, migration, or system-boundary risk.

Before final crystallization, request one final read-only subagent review unless a subagent was already used in the immediately preceding milestone step. Use subagents as advisory reviewers only; their output should be distilled into at most one main-session question or one final-spec adjustment.

Fold only the highest-value finding into the next single question. The panel does not add extra questions, does not decide requirements, and does not permit implementation.

## Closure Gates

Passing the numeric threshold is not enough.

Before writing the final spec:

1. Closure audit: confirm every active component has outcome, scope, constraints, verification, and risk/context coverage. If a material gap remains, explain the gap and ask one more targeted question.
2. Final restatement confirmation: collapse the intended outcome into one sentence and ask the user to confirm whether that sentence alone would lead to the desired result. Use configured-locale wording for this label.

Only crystallize the spec after both gates pass and either:

- the user chooses the milestone option to proceed to `ralplan` at `15%`, `5%`, or `2%`,
- the interview reaches `0%`, or
- the user explicitly exits early with known ambiguity.

If the original user request already asked for the full `deep-interview -> ralplan` pipeline, the crystallized spec should make the next step unmistakable: say that the next assistant turn should start `ralplan` from this locked summary. Do not ask the user for another deep-interview approval once the requested `0%` target is reached.

When the final pending-approval spec is crystallized, output only the user-facing markdown spec. Do not emit `Megara Workflow State`, HTML comments, YAML-like control blocks, JSON, code fences, or any parseable runtime metadata. Runtime hooks infer the crystallized state from the visible final spec and persist runtime state internally.

The final pending-approval spec must be the same assistant response that ends deep-interview. Runtime hooks persist the visible final response as the locked markdown artifact for the interview. A standalone state report is never valid.

End the visible spec with one short configured-locale next-step suggestion. It should tell the user they can continue with `ralplan` from this summary and that implementation is still not allowed. Do not start `ralplan` or implementation in the same response. After the final visible spec, stop; the next assistant turn may begin `ralplan` after the Stop hook persists the lock.

If the user explicitly cancels the interview, say so in normal user-facing prose only. If the interview is still active and asking more questions, keep asking compact visible questions.

## Local Record

Runtime hooks should persist raw prompts and assistant messages locally under `.megara/state/hooks/`.

- `events.jsonl`: append-only hook event index.
- `payloads/<runtime>/<event>/*.json`: append-only raw payload snapshots.
- `last-<runtime>-<event>.json`: convenience pointer to the latest payload only.
- `conversation-events.jsonl`: chronological user/assistant event index.
- `conversation.jsonl`: extracted user prompt and assistant message text when the hook runtime can parse JSON.
- `subagents.jsonl`: observed `SubagentStart` and `SubagentStop` events when the runtime emits them.

When a crystallized final response is visible-only and points to `ralplan` as the next step, runtime hooks persist the visible final response as a markdown lock artifact:

- `.megara/artifacts/deep-interview/specs/deep-interview-<session-id>-<timestamp>.md`
- `.megara/artifacts/deep-interview/specs/index.jsonl`

The matching session JSON should reference `spec_path`, `spec_sha256`, and `spec_persisted_at`.
The matching session JSON should also carry a `pipeline_lock` pointing to `ralplan`; this is runtime state, not visible output.

Do not treat `last-*` files as durable interview history. If a semantic interview ledger is needed, summarize from the conversation history, persisted pending-question state, and append-only hook logs, not from `last-*`.

Do not emit a visible ledger update during active interview turns. Runtime hooks already persist raw prompts, assistant messages, pending questions, answers, and workflow events locally. Put transcript summaries only in the final crystallized lock artifact.

## Output

Produce a user-friendly pending-approval summary, not a raw metadata report.

Visible output should use concise configured-locale headings only:

- Ambiguity: the final percentage. For a `0%` target completion, this must be exactly `0%`.
- Goal: one confirmed sentence plus essential detail.
- Scope: in-scope, out-of-scope, and deferrals.
- Decisions: the important choices made during the interview.
- Acceptance criteria: concrete checks for success.
- Constraints and risks: only items that matter for planning.
- Next step: normally `ralplan`, with a concrete configured-locale sentence.

Do not show raw labels such as `Metadata`, `Clarity breakdown`, `Topology`, `Trigger history`, `Ontology`, `Interview transcript summary`, `spec_path`, `spec_sha256`, `payload`, `persisted_at`, hook event names, or any `Megara ... Gate`/`Megara Workflow State` labels in the visible final response. Internal details belong only in runtime state files managed by hooks.

End in pending approval with visible prose only so the runtime can persist the markdown spec artifact. Do not start implementation from this workflow.
