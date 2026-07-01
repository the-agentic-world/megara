# Megara Harness Source

This directory is the source of truth for Megara's bundled harness.

The installer compiles these files into the `megara` binary, writes them to the selected install scope, and projects them into supported agent runtimes.

## Workflows

- `deep-interview`: Socratic clarification before planning.
- `ralplan`: consensus planning with planner, architect, and critic roles.
- `ultragoal`: durable goal execution with verification gates.
- `team`: multi-agent lane coordination.

## Agents

- `executor`
- `planner`
- `architect`
- `critic`

## Runtime Hooks

- `hooks/megara-hook.sh`: portable hook runner used by runtime adapters to keep lightweight event state without breaking the agent runtime.
