# Homebrew Release

Sisyphus releases are driven by Git tags.

## Required GitHub Settings

Set these in the `the-agentic-world/sisyphus` repository:

- Variable `HOMEBREW_TAP_REPO`: for example `the-agentic-world/homebrew-tap`
- Secret `HOMEBREW_TAP_TOKEN`: a token with write access to that tap repository

The release workflow is still valid without these settings. It will create the GitHub Release assets and skip the Homebrew tap update.

## Release Flow

```bash
git tag v0.0.3
git push origin v0.0.3
```

The workflow builds:

- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`

It uploads `.tar.gz` archives and `.sha256` files to the GitHub Release, then writes `Formula/sisyphus.rb` in the configured Homebrew tap.

## Install

After the tap update:

```bash
brew tap the-agentic-world/tap
brew install sisyphus
```
