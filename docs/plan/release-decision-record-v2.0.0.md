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
- Release commit SHA: `b9d8ffbce9198f5377ad249cca9c673660bb167d` (immutable v2.0.0 tag target)
- Working tree clean: `YES — VB-RELEASE git-status command exit 0`
- Cargo.lock SHA-256: `c97293828b651b69c712904fd3a4e2cb809d05d20467f62817a88ae3258c2f42`
- Target claim: macOS arm64 and Linux x86_64
- Codex CLI host: `0.146.0` on macOS arm64
- Pi host: isolated `0.80.10`; global `/Users/kether/.bun/bin/pi` remains `0.83.0`
- Canonical plan: `docs/plan/planning-core-redesign.md`
- Completion checklist: `docs/plan/planning-core-redesign-completion-checklist.md`
- Evidence archive: [workflow artifact 8875222272](https://github.com/the-agentic-world/megara/actions/runs/30863348608/artifacts/8875222272) — `megara-planning-release-evidence-b9d8ffbce9198f5377ad249cca9c673660bb167d`
- Evidence archive ZIP SHA-256: `83b1442cf462363462d770eee8e0af05e559a758cefd7497887d8449de39c3a8`
- Evidence manifest SHA-256: `33049db1a4c35bceb443fa281716a8bc685af1041fa2b0d1f748eda53c576145`
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
- This baseline manifest is not the final release archive; the recovery run
  regenerated the final archive from immutable release target `b9d8ffbce9198f5377ad249cca9c673660bb167d`.

## 3. Verification bundle status

| Bundle | Evidence | Status |
| --- | --- | --- |
| `VB-MIGRATION-NEGATIVE` | #8 closeout and commit chain through `8642b30c` | PASS before version bump |
| `VB-ADAPTER` | CI 30827545096; Codex/Pi integration and projection checks | PASS before version bump |
| `VB-RELEASE archive (immutable-tag recovery)` | Initial tag run 30861879520 `FAIL` at annotated-tag verification; recovery run 30863348608 `PASS`, artifact 8875222272, manifest/hash receipts | PASS |
| `MPC-MAN-005` | [Codex manual host receipt](release-evidence/codex-manual-2.0.0.md) | PASS isolated fixture |
| `MPC-MAN-006` | [Pi manual host receipt](release-evidence/pi-manual-2.0.0.md) | PASS isolated fixture |

## 4. Blocking or unverified items

| Check | State | Evidence and impact |
| --- | --- | --- |
| Final version/release commit binding | PASS | Cargo.toml/Cargo.lock 2.0.0; evidence and all release assets are bound to immutable target `b9d8ffbce9198f5377ad249cca9c673660bb167d`. |
| Cryptographic Release Decision Record signature | NOT_REQUIRED_BY_EXPLICIT_V2_DECISION | The user approved the unsigned v2.0.0 decision in plain text on 2026-08-04. This exception is release-scoped and is not a PASS. |
| Cryptographic `v2.0.0` tag signature | NOT_REQUIRED_BY_EXPLICIT_V2_DECISION | The tag is annotated, immutable, and pointed at exact release target `b9d8ffbce9198f5377ad249cca9c673660bb167d`. This exception does not allow a lightweight, moved, or force-pushed tag. |
| VB-RELEASE archive (immutable-tag recovery) | PASS | Initial tag run 30861879520 failed at the checkout refspec; recovery run 30863348608 explicitly fetched the immutable annotated tag object and produced artifact 8875222272 with 16 commands, 55 files, 5 traces, all exit 0. |
| GitHub Release assets/checksums | PASS | Release `v2.0.0` contains install.sh and both macOS arm64/Linux x86_64 archives with verified `.sha256` files. |

All required v2.0.0 verification items are now evidenced; no check was skipped
or waived. The two cryptographic signature requirements are explicitly not
required for this release by the user's dated decision and remain visibly
distinct from `PASS`. The release decision is `PASS` for v2.0.0 only.

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

- Final decision: `PASS — 2026-08-04 — v2.0.0`
- Blocking failures/unverified: none
- Explicitly not required: cryptographic RDR/tag signatures under the dated
  v2.0.0 decision; these are not marked PASS
- Unresolved release-scoped TO_DEFINE: none
- Release evidence outcome: exact tag object/target, archive, checksums, smoke
  install, and Homebrew update all PASS
- Release authority decision: `APPROVED IN PLAIN TEXT — 2026-08-04 — v2.0.0 ONLY`

## 7. Immutable-tag recovery receipt

- Initial tag workflow: [run 30861879520](https://github.com/the-agentic-world/megara/actions/runs/30861879520) — `FAIL` at annotated-tag verification before build or publish.
- Failure cause: `actions/checkout@v4` fetched the tagged commit directly into `refs/tags/v2.0.0`, so the runner saw a `commit` ref instead of the remote annotated `tag` object.
- Immutable remote tag retained: object `0151c363af61c27b9a2fc23724956cb7d2481a2b`, peeled target `b9d8ffbce9198f5377ad249cca9c673660bb167d`; no deletion, movement, or recreation is permitted.
- Recovery workflow: [run 30863348608](https://github.com/the-agentic-world/megara/actions/runs/30863348608) — `PASS`.
- Dispatch inputs: `release_tag=v2.0.0`, `release_commit=b9d8ffbce9198f5377ad249cca9c673660bb167d`, `release_tag_object=0151c363af61c27b9a2fc23724956cb7d2481a2b`, `main_at_release=b9d8ffbce9198f5377ad249cca9c673660bb167d`.
- Verify job 91849838343 confirmed remote/local object type `tag`, object SHA `0151c363af61c27b9a2fc23724956cb7d2481a2b`, peeled target `b9d8ffbce9198f5377ad249cca9c673660bb167d`, full-history ancestor relation, and output bindings.
- Build jobs 91850346689 and 91850346715, publish 91850693255, smoke jobs 91850720630/91850720645, Homebrew job 91850752234, and prune job 91850752242 all `PASS`.
- GitHub Release: [v2.0.0](https://github.com/the-agentic-world/megara/releases/tag/v2.0.0). Assets/checksums: `install.sh` digest `161b1e8abc31a41cd1906995e2b1733225f99c25a4e644d3e3e022d840f5e923`; macOS archive `bccbdb709e41a2a77c935d838a188b0b38d6a86190265faf9411010b3ac11dc5`; macOS checksum asset `3d21baa5a8734e9650843ab21c807a8665b26702acb1622048947dec7774fa4a`; Linux archive `51a96d2b752819a1150ad68a61f1ebaab00b2fb2131e1fb98795a80925d5286f`; Linux checksum asset `ec525d19c9251f21a887a1d3128bb4a841ab39bf14a231a3a484c49c9fbda3f4`.
- Tag receipt: remote/local `refs/tags/v2.0.0` object `0151c363af61c27b9a2fc23724956cb7d2481a2b`, type `tag`, peeled target `b9d8ffbce9198f5377ad249cca9c673660bb167d`; `git tag -v` exits 1 with `no signature found` as required by the explicit v2-only unsigned decision. No tag deletion, movement, recreation, or force push occurred.
