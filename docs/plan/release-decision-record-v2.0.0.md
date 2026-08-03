---
type: DecisionRecord
title: Megara 2.0.0 Release Decision Record
description: Release gate record for Megara 2.0.0, including the explicit unsigned-release decision and final tag/archive binding.
timestamp: 2026-08-04
tags: [okf, decision, release, planning, v2]
---

# Megara 2.0.0 Release Decision Record

This record follows the Release Decision Record contract in
`docs/plan/planning-core-redesign-completion-checklist.md` and the operating
procedure in `docs/plan/planning-core-v1-release-operations.md`. The user made
an explicit, release-scoped decision on 2026-08-04 that v2.0.0 will not use a
cryptographic Release Decision Record signature or cryptographically signed
tag. This is a plain-text approved decision, not a claim that a signature
exists, and it does not change the default policy for future releases.

The release may be marked `PASS` only after every remaining blocking and manual
gate is tied to the final main commit and immutable tag archive. The two
signature gates are recorded as `NOT_REQUIRED_BY_EXPLICIT_V2_DECISION`, never
as `PASS` or as a silent waiver.

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
- Tag form: annotated tag, cryptographically unsigned by explicit v2.0.0 decision

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
| Cryptographic Release Decision Record signature | NOT_REQUIRED_BY_EXPLICIT_V2_DECISION | The user approved the unsigned v2.0.0 decision in plain text on 2026-08-04. This exception is release-scoped and is not a PASS. |
| Cryptographic `v2.0.0` tag signature | NOT_REQUIRED_BY_EXPLICIT_V2_DECISION | The final tag must still be annotated, immutable, and pointed at the exact final main SHA. This exception does not allow a lightweight, moved, or force-pushed tag. |
| Tag-triggered `VB-RELEASE` archive | PENDING | The archive must run from the exact annotated tag target and retain all raw evidence and hashes. |
| GitHub Release assets/checksums | PENDING | Cannot verify before the tag workflow completes. |

No remaining verification item is skipped or waived. The two cryptographic
signature requirements are explicitly not required for this release by the
user's dated decision and remain visibly distinct from `PASS`. Until the final
tag, archive, and assets are verified, the release decision is `FAIL/PENDING`,
and #4/#11 must remain open.

## 5. Manual reviewer authorization and fixture boundary

The user explicitly authorized the Megara 2.0.0 release and, for the isolated
temporary fixture only, authorized submission after exact screen comparison.
On 2026-08-04 the user additionally approved the v2.0.0 unsigned-release
policy in plain text, replacing the prior signed RDR/tag gate only for this
release.
The Codex fixture matched the precomputed session, candidate ID, semantic hash,
revision, command ID, and argv before approval; the purge fixture matched the
session identity, revision, command ID, and argv before purge. The Pi 0.80.10
fixture recorded the equivalent visible values. These receipts do not authorize
mutations to a real user project or production state.

## 6. Final decision

- Final decision: `PENDING — NOT PASS`
- Blocking failures/unverified: final annotated tag, exact main target, archive,
  and release assets
- Explicitly not required: cryptographic RDR/tag signatures under the dated
  v2.0.0 decision; these are not marked PASS
- Unresolved TO_DEFINE: must be reconciled against the completion checklist
  before issue closeout
- Release authority decision: `APPROVED IN PLAIN TEXT — 2026-08-04 — v2.0.0 ONLY`

## 7. Immutable-tag recovery receipt

- Initial tag workflow: [run 30861879520](https://github.com/the-agentic-world/megara/actions/runs/30861879520) — `FAIL` at annotated-tag verification before build or publish.
- Failure cause: `actions/checkout@v4` fetched the tagged commit directly into `refs/tags/v2.0.0`, so the runner saw a `commit` ref instead of the remote annotated `tag` object.
- Immutable remote tag retained: object `0151c363af61c27b9a2fc23724956cb7d2481a2b`, peeled target `b9d8ffbce9198f5377ad249cca9c673660bb167d`; no deletion, movement, or recreation is permitted.
- Recovery workflow: `PENDING fix merge and dispatch`. It must require and compare `release_tag`, peeled `release_commit`, recorded `release_tag_object`, and `main_at_release`; it must explicitly fetch the remote tag object, bind all build/evidence/publish inputs to the peeled target, and require that target to be an ancestor of current `main`.
