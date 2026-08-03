---
type: DecisionRecord
title: Megara 2.0.0 Release Decision Record
description: Release gate record for Megara 2.0.0, pending final main, tag, and archive binding.
timestamp: 2026-08-04
tags: [okf, decision, release, planning, v2]
---

# Megara 2.0.0 Release Decision Record

This record follows the Release Decision Record contract in
`docs/plan/planning-core-redesign-completion-checklist.md` and the operating
procedure in `docs/plan/planning-core-v1-release-operations.md`. It is
intentionally not marked `PASS` until every blocking and manual gate is tied to
the final main commit and the signed tag archive.

## 1. Release identity

- Release version: `2.0.0`
- Release commit SHA: `PENDING final main integration`
- Working tree clean: `PENDING final release commit`
- Cargo.lock SHA-256: `PENDING final release commit`
- Target claim: macOS arm64 and Linux x86_64
- Codex CLI host: `0.146.0` on macOS arm64
- Pi host: isolated `0.80.10`; global `/Users/kether/.bun/bin/pi` remains `0.83.0`
- Canonical plan: `docs/plan/planning-core-redesign.md`
- Completion checklist: `docs/plan/planning-core-redesign-completion-checklist.md`
- Evidence archive: `PENDING tag workflow artifact megara-planning-release-evidence-<release-commit>`
- Evidence archive SHA-256: `PENDING tag workflow`

## 2. Baseline and automated evidence

- Baseline/release candidate before the 2.0.0 version bump:
  `8642b30c51da2dd50edbcb771df1efbb47967833`.
- Successful cross-platform CI: [Actions run 30827545096](https://github.com/the-agentic-world/megara/actions/runs/30827545096).
- That run passed canonical Pi setup, Linux x86_64 build/smoke, macOS arm64
  build/smoke, Rust checks, docs, and release scripts.
- The local baseline manifest is
  `target/megara-evidence/8642b30c51da2dd50edbcb771df1efbb47967833/manifest.json`
  (schema v1, clean gate, 16 commands, 5 traces, 55 hashes, all exit 0).
- This baseline manifest is not the final release archive; it must be
  regenerated on the final release commit.

## 3. Verification bundle status

| Bundle | Evidence | Status |
| --- | --- | --- |
| `VB-MIGRATION-NEGATIVE` | #8 closeout and commit chain through `8642b30c` | PASS before version bump |
| `VB-ADAPTER` | CI 30827545096; Codex/Pi integration and projection checks | PASS before version bump |
| `VB-RELEASE` | `scripts/release-evidence.sh` baseline manifest; tag workflow required | PENDING final commit/tag |
| `MPC-MAN-005` | [Codex manual host receipt](release-evidence/codex-manual-2.0.0.md) | PASS isolated fixture |
| `MPC-MAN-006` | [Pi manual host receipt](release-evidence/pi-manual-2.0.0.md) | PASS isolated fixture |

## 4. Blocking or unverified items

| Check | State | Evidence and impact |
| --- | --- | --- |
| Final version/release commit binding | PENDING | Must be regenerated after main integration. |
| Signed Release Decision Record | PENDING | User supplied reviewer authorization for the isolated fixtures; final release authority signature is not yet recorded. |
| Signed `v2.0.0` tag | BLOCKED | This host has no `gpg` executable, no configured signing format/key, and no SSH signing identity. An unsigned tag is not an acceptable substitute. |
| Tag-triggered `VB-RELEASE` archive | PENDING | Cannot run before the signed tag and final clean commit. |
| GitHub Release assets/checksums | PENDING | Cannot verify before the tag workflow completes. |

No item above is skipped or waived. Until these states are resolved, the final
release decision is `FAIL/PENDING`, and #4/#11 must remain open.

## 5. Manual reviewer authorization and fixture boundary

The user explicitly authorized the Megara 2.0.0 release and, for the isolated
temporary fixture only, authorized submission after exact screen comparison.
The Codex fixture matched the precomputed session, candidate ID, semantic hash,
revision, command ID, and argv before approval; the purge fixture matched the
session identity, revision, command ID, and argv before purge. The Pi 0.80.10
fixture recorded the equivalent visible values. These receipts do not authorize
mutations to a real user project or production state.

## 6. Final decision

- Final decision: `PENDING — NOT PASS`
- Blocking failures/unverified: at least the signed tag and final archive gates
- Manual failures/unverified: final release-authority signature and final
  release-commit binding
- Unresolved TO_DEFINE: must be reconciled against the completion checklist
  before issue closeout
- Release authority signature: `PENDING`
