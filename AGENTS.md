# Megara

Megara is a Rust CLI for installing project-level or global agent harnesses.

## Rules

- Keep the CLI small and deterministic.
- Do not reintroduce Sisyphus issue-broker, daemon, queue, polling, auth, TUI, or worktree code.
- `src/templates.rs` is the built-in harness source for v1.
- `src/targets/codex.rs` owns Codex projection behavior.
- Default write behavior must protect existing user files unless `--force` is supplied.
