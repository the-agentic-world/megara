---
type: Evidence
title: Megara 2.0.0 Pi manual host receipt
description: Isolated Pi 0.80.10 confirmation and purge evidence for the 2.0.0 release gate.
timestamp: 2026-08-04
tags: [okf, evidence, release, pi, planning]
---

# Megara 2.0.0 Pi manual host receipt

## Scope and version boundary

The canonical Pi contract is `>=0.80.10` and `<0.81.0`. The installed global
Pi `/Users/kether/.bun/bin/pi` is `0.83.0` and was preserved without downgrade
or overwrite. This gate used only the isolated official package:

- package: `@earendil-works/pi-coding-agent@0.80.10`;
- executable: `/Users/kether/.Trash/megara-pi-compat-test-0.80.10-20260804/node_modules/.bin/pi`;
- fixture root: `/var/folders/j4/vqb23wkd5mg95130kd__12t80000gn/T/megara-release-host-matrix-ew3msmv3`;
- project: `/var/folders/j4/vqb23wkd5mg95130kd__12t80000gn/T/megara-release-host-matrix-ew3msmv3/project`;
- source receipt: `/var/folders/j4/vqb23wkd5mg95130kd__12t80000gn/T/megara-release-host-matrix-ew3msmv3/pi-manual-receipt.json`.

The source receipt records Pi integration `3/3` PASS, direct RPC command
discovery, and project install projection. The global 0.83.0 installation was
not modified.

## Approval and purge screen evidence

The Pi fixture session was
`pln_019fc862-ae10-76e2-aee3-8d47e6c0582b`. The candidate shown in the approval
screen was:

- candidate: `cand_spec_019fc862-b078-7700-a9a5-0851b724f7ad`;
- semantic hash: `sha256:855b35717eb5ce3f734ed2f77bf12b39a67f0abca78d1d0231273ead03c57948`;
- approval revision: `5`;
- base domain revision: `3`;
- approval event sequence: `6`;
- base revision after approval: `3` → `6`.

The confirmation UI showed the same candidate ID, hash, revision, session, and
exact command argv before the confirmation response `success: true`. The raw
approval notification in the source receipt has command ID
`cmd_019fc862-b450-7dc2-bbf1-5149a513c523`, request ID
`req_019fc862-b450-7dc2-bbf1-513a855dfbe2`, `ok: true`,
`evidence_current: true`, `projection_status: "unchanged"`, empty warnings,
and `replayed: false`.

The purge screen showed the same session as both `session_id` and `confirm`,
with expected revision `6` and the exact purge argv. Its raw notification has
command ID `cmd_019fc862-b72c-7e22-bbd9-82d8fa2d5e63`, request ID
`req_019fc862-b72c-7e22-bbd9-82cfbe8f3111`, `ok: true`,
`cleanup_state: "clean"`, `purged: true`, and `projection_status: "unchanged"`.
The post-purge list check returned no sessions. No production or user-project
state was involved.

## Gate result

`MPC-MAN-006` is satisfied for the isolated Pi 0.80.10 host fixture: target
identity, semantic hash, and revision were human-visible and matched the
executed command. The release remains unbound until the final release commit,
plain-text approved RDR decision, annotated immutable unsigned tag, and
immutable tag workflow archive are verified. Cryptographic RDR/tag signatures
are not required for v2.0.0 by the dated explicit decision only.
