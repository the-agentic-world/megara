---
name: ultragoal
description: Durable goal execution workflow with checkpoints and verification gates
argument-hint: "<approved goal or plan>"
handoff-policy: approved-execution-only
---

# Ultragoal

Use this workflow when an approved plan should be executed to completion with durable progress tracking.

## Contract

- Start only from an approved goal or plan.
- Break the goal into independently verifiable stories when needed.
- Keep one active story at a time unless the work is explicitly split into independent lanes.
- Record evidence before considering a story complete.
- Treat missing tests, shallow evidence, or plan/code mismatches as blockers.
- Do not ask the user to resolve work the agent can investigate or fix.
- Write every user-facing sentence in the configured locale, including progress updates, active story reports, verification notes, blocker reports, and final summaries.
- Keep file paths, commands, config keys, API names, and quoted source text unchanged.
- Before sending a response, replace stock English workflow phrases with configured-locale prose. Do not mix languages in explanatory prose.

## Execution Loop

1. Restate the active goal and done criteria.
2. Select the next story.
3. Implement the smallest correct change.
4. Run focused verification.
5. Run cleanup and review gates.
6. Record completion evidence or a concrete blocker.
7. Continue until every story is complete.

## Completion Gate

Before marking work complete:

- Run relevant tests or live-surface checks.
- Review changed files for slop, hidden fallback behavior, dead code, needless abstraction, boundary violations, and missing tests.
- Run architecture or critic review for high-risk changes.
- Report exact evidence.
