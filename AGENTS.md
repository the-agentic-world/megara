# Megara

Megara is a Rust CLI for installing project-level or global agent harnesses.

## Rules

- Keep the CLI small and deterministic.
- Do not reintroduce legacy issue-broker, daemon, queue, polling, auth, TUI, or worktree code.
- `.agents/` is the built-in harness source for v1.
- `src/templates.rs` only indexes tracked `.agents/` files into the binary.
- `src/targets/codex.rs` owns Codex projection behavior.
- Default write behavior must protect existing user files unless `--force` is supplied.
