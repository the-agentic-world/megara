# Megara Harness Source

`harness/` is the bundled source for the installable Megara harness. The
installer writes these files to the selected `.agents/` or `~/.megara/` scope
and projects supported agent-runtime files from the same source of truth.

## Configuration

- `megara.toml` is the harness configuration source.
- `locale` controls user-facing response language in projected runtimes.
- Technical literals such as paths, commands, config keys, package names, and
  quoted source text stay unchanged when explanatory prose is translated.
- Structured block keys remain parseable; free-text values follow the configured
  locale unless they are technical literals.

## Skills, tools, and agents

- `caveman` is an always-on terse response compression skill.
- `insane-search` is an on-demand public web access tool and skill wrapper.
- Agent files define the supported planning/review roles and their model
  policies.
- Tool state and dependencies belong under `.megara/state/tools/<tool>` or the
  tool's own cache paths; they are not planning state.

## Planning

- Planning state is project-local under `.megara/planning`.
- The deterministic Planning Core owns state, transitions, approvals, and
  persistence.
- Runtime adapters submit typed requests and display returned work items; they
  do not write the planning database directly.
- Approval and purge operations require an explicit user entrypoint.
- Megara never commits or pushes changes automatically.

## Runtime boundaries

- Installation, synchronization, and doctor are deterministic command-scoped
  adapters around the installer and Planning Core.
- Runtime data remains separate from source and managed projection files.
- `.megara/state/tools`, `.megara/cache`, `.megara/planning`, and migration
  backup data are protected by the runtime ignore rules.
