---
type: Evidence
title: Megara 2.0.0 evidence archive input contract
description: Release archive contents and final binding requirements for VB-RELEASE.
timestamp: 2026-08-04
tags: [okf, evidence, release, archive, vb-release]
---

# Megara 2.0.0 evidence archive input contract

The authoritative archive is produced by the tag-triggered
`.github/workflows/release.yml` workflow. The local `target/megara-evidence`
directory is staging only and is never treated as the final archive.

## Required binding

The final archive must be generated from the exact clean commit targeted by
`v2.0.0` and must contain the `megara.release-evidence/v1` manifest, the raw
command stdout/stderr, `test-report.txt`, DB/FS/protocol/migration/purge
traces, command exit codes, and SHA-256 hashes for every file except the
manifest itself. The workflow upload name is:

```text
megara-planning-release-evidence-<release-commit>
```

The retention period is 90 days and `if-no-files-found: error` is required.
Source or `Cargo.lock` changes after evidence generation invalidate the bundle.

## Current inputs

- baseline release candidate: `8642b30c51da2dd50edbcb771df1efbb47967833`;
- successful CI baseline: [run 30827545096](https://github.com/the-agentic-world/megara/actions/runs/30827545096);
- baseline local manifest: `target/megara-evidence/8642b30c51da2dd50edbcb771df1efbb47967833/manifest.json`;
- Codex manual host receipt: [codex-manual-2.0.0.md](codex-manual-2.0.0.md);
- Pi manual host receipt: [pi-manual-2.0.0.md](pi-manual-2.0.0.md).

The baseline manifest is evidence input, not the final `VB-RELEASE` archive:
the final tag workflow must regenerate it after the 2.0.0 version and release
documents are integrated. The final RDR must record the tag workflow URL,
artifact URL, artifact hash/manifest, and release asset checksums.
