# Megara

[![CI](https://github.com/the-agentic-world/sisyphus/actions/workflows/ci.yml/badge.svg)](https://github.com/the-agentic-world/sisyphus/actions/workflows/ci.yml)
[![Release](https://github.com/the-agentic-world/sisyphus/actions/workflows/release.yml/badge.svg)](https://github.com/the-agentic-world/sisyphus/actions/workflows/release.yml)

Megara installs a portable agent harness at project or global scope, then projects it into supported agent runtimes. It borrows the installation shape of OMA while keeping its own SSOT and templates.

V1 targets Codex only, but the code is structured around target adapters so additional agent runtimes can be added without changing the installer contract.

## Install

```bash
brew install the-agentic-world/tap/megara
```

## Usage

Run the installer wizard:

```bash
megara install
```

Non-interactive project install:

```bash
megara install --scope project --target codex
```

Global install:

```bash
megara install --scope global --target codex
```

Check installation health:

```bash
megara doctor --scope project --target codex
```

Reproject managed files from the SSOT:

```bash
megara sync --scope project --target codex
```

List templates and targets:

```bash
megara templates list
megara targets list
```

## Scope Model

- `project`: writes the SSOT to `.agents/` in the current project and projects Codex files to `.codex/`.
- `global`: writes the SSOT to `~/.megara` and projects Codex files to `~/.codex`.

Megara protects existing files by default. If a destination file exists and is not Megara-managed, the command reports a conflict and leaves it untouched. Use `--force` only when you intentionally want Megara to take ownership.

## Included Harness

Workflows:

- `deep-interview`
- `ralplan`
- `ultragoal`
- `team`

Role agents:

- `executor`
- `planner`
- `architect`
- `critic`
