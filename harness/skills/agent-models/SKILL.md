---
name: agent-models
description: Review and safely update Megara role model policies
argument-hint: "[role or policy goal]"
---

# Agent Models

Use this skill to review or change role-specific model and reasoning policies.

## Rules

- Inspect the effective policy with `megara agents show` before proposing a change.
- Explain the quality, latency, and cost tradeoff for each changed role.
- Never change a policy without an explicit user approval in the current conversation.
- Apply approved changes through `megara agents configure`; never edit generated runtime agent files.
- Verify the result with `megara agents show` after applying it.

## Workflow

1. Identify the scope, runtime, and affected roles.
2. Show the current effective policy.
3. Propose a focused policy change with a recommendation and rationale.
4. Wait for explicit approval.
5. Run `megara agents configure` with the approved values.
6. Confirm the resulting effective policy.
