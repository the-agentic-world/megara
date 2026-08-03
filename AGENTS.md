# Megara

Megara is a Rust CLI for installing project-level or global agent harnesses.

## Rules

- Keep the CLI small and deterministic.
- Do not reintroduce legacy issue-broker, daemon, queue, polling, auth, worktree code, or dashboard/daemon TUI surfaces.
- Command-scoped `ratatui` screens for `install`, `update`, and `doctor` are allowed only as input/presentation adapters around the existing deterministic CLI logic.
- `harness/` is the built-in harness source for v1.
- `src/templates.rs` only indexes tracked `harness/` files into the binary.
- `src/targets/codex.rs` owns Codex projection behavior.
- Default write behavior must protect existing user files unless `--force` is supplied.

## GitHub Issues

- GitHub Issues are the authoritative execution tracker for every plan, bug, feature, refactor, maintenance task, and release task in this repository.
- Before substantive planning or implementation, search the relevant open and closed issues, then link the work to an existing issue or create one. Do not create duplicate issues.
- Track umbrella plans in a parent issue. Track executable units in linked child issues or in an explicit checklist attached to the parent issue.
- Keep each issue's scope, acceptance criteria, dependencies and blockers, and verification evidence current.
- Synchronize issue checkboxes, status, and evidence with the actual repository state throughout the work.
- Do not close an issue until its acceptance and verification criteria pass. Reopen it if a regression invalidates the completion evidence.
- Reference related issues from branches, commits, and pull requests.
- Local documents and chat may contain detailed specifications or evidence, but they are not the sole execution tracker.
- If GitHub access or write permission is unavailable, report the work as blocked; do not replace GitHub Issues with a local-only tracker.
- These operating rules do not add a product-runtime issue broker, daemon, queue, or polling system; the existing prohibition on those surfaces remains in force.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

When the user types `/graphify`, use the installed graphify skill or instructions before doing anything else.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- Dirty graphify-out/ files are expected after hooks or incremental updates; dirty graph files are not a reason to skip graphify. Only skip graphify if the task is about stale or incorrect graph output, or the user explicitly says not to use it.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
