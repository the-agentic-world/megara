---
name: deep-interview
description: Socratic requirements interview with ambiguity scoring before execution approval
argument-hint: "[--quick|--standard|--deep] <idea or vague description>"
pipeline: [deep-interview, ralplan]
handoff-policy: approval-required
---

# Deep Interview

Use this workflow when the user's request is broad, vague, high-risk, or likely to hide assumptions.

## Contract

- Do not implement during deep-interview.
- Ask one question at a time.
- Inspect repository facts before asking the user about facts the repository can answer.
- Start with a topology confirmation: identify the top-level components or outcomes and ask whether the shape is correct.
- Score ambiguity after each answer and name the weakest dimension.
- Continue until the remaining ambiguity is low enough to produce a useful specification or the user explicitly exits early.
- End with a pending-approval specification and a recommendation to continue through `ralplan`.

## Ambiguity Dimensions

- Outcome clarity: what must be true at the end.
- Scope boundary: what is in and out.
- User value: who benefits and why.
- Technical constraints: runtime, data, integration, and migration limits.
- Verification: how success will be proven.
- Risk: irreversible, destructive, security, privacy, or operational concerns.

## Output

Produce a concise specification:

- Goal
- Confirmed topology
- In scope / out of scope
- Decisions made
- Open questions or accepted assumptions
- Acceptance criteria
- Suggested next workflow
