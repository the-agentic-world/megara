---
name: team
description: Multi-agent lane coordination for approved work that benefits from parallel roles
argument-hint: "<approved task with lanes or separable workstreams>"
handoff-policy: approved-execution-only
---

# Team

Use this workflow when approved work benefits from multiple coordinated roles. The current session is always the team leader.

## Contract

- Do not use team to discover basic scope; use `deep-interview` or `ralplan` first.
- Launch team only when the work has separable lanes or needs visible role-based review.
- Keep one leader responsible for integration and final verification.
- Give each lane a bounded task, allowed files or surfaces, acceptance criteria, and evidence requirements.
- Merge lane results only after conflicts and verification are settled.
- Select two to four teammate roles from the task. Simple work normally uses `executor` and `critic`. Add `planner` for sequencing risk. Add `architect` for boundary, integration, runtime, migration, or adapter risk.
- Each teammate assignment and result must carry a correlation id and teammate id.
- Duplicate, malformed, out-of-order, or unrelated teammate output is not enough to finish. Ask for a corrected teammate result or mark a teammate failure and explain the impact.
- Final synthesis is allowed only after every required teammate has returned a result or failure.
- Write every user-facing sentence in the configured locale, including progress updates, lane assignments, status reports, review findings, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- For technical concepts, prefer natural terms in the configured locale when they exist. If an English-only technical term must remain, add a short explanation in the configured locale on first use, preferably as a footnote or parenthetical note.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.
- Do not show raw runtime metadata, hook fields, state paths, sha256 values, or workflow gate labels in visible prose. Runtime metadata is internal-only.

## Default Lanes

- Delivery: implementation by `executor`.
- Verification: test and adversarial checks by `executor` or `critic`.
- Architecture: boundary and tradeoff review by `architect`.
- Planning support: sequencing or handoff refinement by `planner`.

## Codex App

- The current App thread is the team leader.
- Use Codex subagents as teammates.
- The leader assigns bounded work, waits for teammate results, resolves conflicts, then synthesizes the final response.

## Codex CLI

- The current CLI session is the team leader.
- Prefer Warp pane teammates only when stable pane creation and message exchange are available.
- Target Warp layout: two columns, leader on the left, right column split into one row per teammate.
- If Warp pane creation or message exchange fails, print exactly one fallback notice and use Codex subagents instead:

```text
Warp pane 생성 실패로 subagent fallback 사용
```

## Message Contract

- `assignment`: leader gives a bounded teammate task.
- `teammate-status`: teammate reports progress or a blocker.
- `teammate-result`: teammate returns usable evidence.
- `teammate-failure`: teammate cannot complete the assignment.
- `leader-synthesis`: leader integrates teammate outputs.
- `fallback-notice`: leader reports Warp fallback when needed.

## Output

Return teammate status, integration notes, verification evidence, and remaining blockers. Keep runtime metadata hidden.
