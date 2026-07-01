---
name: team
description: Multi-agent lane coordination for approved work that benefits from parallel roles
argument-hint: "<approved task with lanes or separable workstreams>"
handoff-policy: approved-execution-only
---

# Team

Use this workflow when approved work benefits from multiple coordinated roles.

## Contract

- Do not use team to discover basic scope; use `deep-interview` or `ralplan` first.
- Launch team only when the work has separable lanes or needs visible role-based review.
- Keep one leader responsible for integration and final verification.
- Give each lane a bounded task, allowed files or surfaces, acceptance criteria, and evidence requirements.
- Merge lane results only after conflicts and verification are settled.
- Write every user-facing sentence in the configured locale, including progress updates, lane assignments, status reports, review findings, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.

## Default Lanes

- Delivery: implementation by `executor`.
- Verification: test and adversarial checks by `executor` or `critic`.
- Architecture: boundary and tradeoff review by `architect`.
- Planning support: sequencing or handoff refinement by `planner`.

## Output

Return lane status, integration notes, verification evidence, and remaining blockers.
