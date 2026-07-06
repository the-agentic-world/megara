# Megara

Megara is a Rust CLI for installing project-level or global agent harnesses.

## Rules

- Keep the CLI small and deterministic.
- Do not reintroduce legacy issue-broker, daemon, queue, polling, auth, TUI, or worktree code.
- `harness/` is the built-in harness source for v1.
- `src/templates.rs` only indexes tracked `harness/` files into the binary.
- `src/targets/codex.rs` owns Codex projection behavior.
- Default write behavior must protect existing user files unless `--force` is supplied.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

When the user types `/graphify`, use the installed graphify skill or instructions before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Dirty graphify-out/ files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
