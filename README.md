<p align="center">
  <img src="docs/assets/readme-banner.png" alt="Sisyphus from Greek mythology pushing a boulder up Mount Olympus" width="100%">
</p>

<h1 align="center">Sisyphus</h1>

<p align="center">
  Local issue-to-agent broker and lifecycle controller.
</p>

<p align="center">
  <a href="https://github.com/the-agentic-world/sisyphus/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/the-agentic-world/sisyphus/ci.yml?branch=main&label=ci&style=flat-square"></a>
  <a href="https://github.com/the-agentic-world/sisyphus/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/the-agentic-world/sisyphus/release.yml?label=release&style=flat-square"></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-2024-CE412B?style=flat-square&logo=rust&logoColor=white">
  <img alt="Local first" src="https://img.shields.io/badge/local--first-no%20central%20backend-2F6F6A?style=flat-square">
  <img alt="Providers" src="https://img.shields.io/badge/providers-GitHub%20%7C%20GitLab-3D6FA8?style=flat-square">
  <img alt="Agent" src="https://img.shields.io/badge/agent-Codex-1F2937?style=flat-square">
</p>

Sisyphus watches issue trackers, converts issue activity into agent-friendly work items, and dispatches that work into Codex without making the agent spend context discovering tasks.

It is intentionally local-first: no hosted Sisyphus backend, no central queue, and no custom agent session database. Sisyphus owns provider polling, queue state, lifecycle events, clarification comments, and dispatch bookkeeping. Codex still owns Codex sessions.

## Status

Sisyphus is an early MVP. The current implementation supports:

- GitHub and GitLab issue import and polling
- local SQLite queue and lifecycle event storage under `~/.sisyphus`
- Unix socket daemon API
- TUI/dashboard entry point
- Codex capability probing and dispatch
- clarification-question comments back to the issue provider
- foreground daemon, background daemon, and macOS LaunchAgent registration
- release artifacts for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`

## Why

Agent tools should execute work, not burn tokens scanning issue lists, guessing which label matters, or rediscovering repository context. Sisyphus turns issue tracker events into explicit work packets:

```text
GitHub / GitLab issue
  -> Sisyphus polling loop
  -> normalized WorkItem
  -> clarification gate
  -> Codex-native session
  -> provider comments and local lifecycle events
```

If an issue is ambiguous, Sisyphus asks the agent to produce clarification questions first, then posts those questions as provider comments. Raw issue text is not treated as execution-ready work by default.

## Install

### Homebrew

Homebrew distribution is wired through the release workflow. After the tap is configured and the first release is published:

```bash
brew tap the-agentic-world/tap
brew install sisyphus
```

### From Source

```bash
git clone https://github.com/the-agentic-world/sisyphus.git
cd sisyphus
cargo install --path .
```

## Quick Start

Start the local backend:

```bash
sisyphus serve
```

Or run it headlessly:

```bash
sisyphus serve --daemon
```

Open the dashboard:

```bash
sisyphus
```

Authenticate a provider locally, then register a provider polling target:

```bash
sisyphus auth github --client-id <github-oauth-client-id>
sisyphus provider-add github the-agentic-world sisyphus
```

GitHub OAuth uses Device Flow and requires an explicit OAuth App client ID:

```bash
sisyphus auth github --client-id <github-oauth-client-id>
sisyphus auth github --client-id <github-oauth-client-id> --scope public_repo
```

To authenticate GitHub with a personal access token instead, skip `sisyphus auth
github` and register the polling target with `--token-env`. When `--token-env` is
set, Sisyphus reads that environment variable and does not read the stored OAuth
token:

```bash
export GITHUB_TOKEN=...
sisyphus provider-add github the-agentic-world sisyphus --token-env GITHUB_TOKEN
```

GitLab currently uses local token input:

```bash
sisyphus auth gitlab
```

Map the provider repository to a local workspace:

```bash
sisyphus repo-add github the-agentic-world sisyphus /path/to/sisyphus
```

Import or inspect work:

```bash
sisyphus import https://github.com/the-agentic-world/sisyphus/issues/1
sisyphus queue
sisyphus dispatch 1 --dry-run
```

Probe Codex integration:

```bash
sisyphus codex-probe
```

Register reboot-persistent autostart on macOS:

```bash
sisyphus register
```

## Configuration

Sisyphus stores configuration at:

```text
~/.sisyphus/config.toml
```

Default polling is every 5 seconds with capped backoff:

```toml
[polling]
interval_seconds = 5
max_backoff_seconds = 60

[dispatch]
auto_dispatch = true
require_open = true
trigger_labels = ["sisyphus"]
ignore_labels = ["wontfix", "blocked"]
```

Provider tokens are stored in the OS credential store with `sisyphus auth github` or
`sisyphus auth gitlab` where supported. GitHub OAuth requires `--client-id`.
Environment variable PATs are supported with `provider-add --token-env` and take
precedence over stored credentials; raw token values are not written to the config.

## CLI

```text
sisyphus                         Open the local dashboard
sisyphus serve                   Run the backend in the foreground
sisyphus serve --daemon          Run the backend headlessly
sisyphus register                Register macOS LaunchAgent autostart
sisyphus auth github --client-id <id> Store GitHub auth via OAuth Device Flow
sisyphus auth gitlab             Store a GitLab token in the OS credential store
sisyphus provider-add ...        Register a provider repository polling target
sisyphus repo-add ...            Map a provider repository to a local path
sisyphus import <issue-url>      Import an issue into the queue
sisyphus queue                   List queued work
sisyphus dispatch <id>           Dispatch queued work to Codex
sisyphus sessions                List Codex session references
sisyphus events                  List lifecycle events
sisyphus open <id>               Open or print a Codex-native session reference
sisyphus codex-probe             Probe Codex integration capabilities
```

## Architecture

```text
Issue Provider Adapters
  GitHub / GitLab
        |
        v
Local Daemon
  polling loop
  queue
  lifecycle engine
  clarification comments
  Unix socket control API
        |
        v
Agent Adapter
  Codex capability probing
  Codex-native dispatch
  session refs only
```

Sisyphus does not implement a central backend and does not take over agent session management. It stores only the references needed to reopen or report on native agent sessions.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --all-targets --all-features
```

The CI workflow runs the same Rust checks on Ubuntu and macOS arm runners.

## Release

Releases are tag-driven:

```bash
git tag v0.0.5
git push origin v0.0.5
```

The release workflow builds:

- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`

See [docs/release/homebrew.md](docs/release/homebrew.md) for Homebrew tap setup.

## Design Constraints

- local daemon only
- polling, not webhooks
- no central Sisyphus backend
- Unix socket control API first
- Codex first, more agents later
- agent sessions remain native to the agent runtime
- repository harnesses such as OMA are executed by the agent, not by Sisyphus
