---
type: Evidence
title: Megara 2.0.0 Codex manual host receipt
description: Isolated Codex CLI confirmation and purge evidence for the 2.0.0 release gate.
timestamp: 2026-08-04
tags: [okf, evidence, release, codex, planning]
---

# Megara 2.0.0 Codex manual host receipt

## Scope and safety boundary

This receipt covers an isolated temporary fixture only. It is not a production or
user-project approval. The user's explicit 2.0.0 release approval was used as
reviewer authorization only after every displayed candidate, hash, revision,
session identity, command ID, and exact argv matched the values calculated before
the input was submitted.

- Host: macOS arm64; Codex CLI `0.146.0`.
- Fixture project: `/var/folders/j4/vqb23wkd5mg95130kd__12t80000gn/T/megara-release-host-matrix-ew3msmv3/project`.
- Fixture baseline: `66db1684fcdb20dbe4d55ce8668b5eb1b417345c`, `fixture: pin installed Codex projection`.
- Megara executable used by the fixture MCP server: `/Users/kether/.codex/worktrees/61c/megara/target/debug/megara`.
- Fixture state was clean before the host interaction and the Codex TUI was shut down with a bounded EOF after the final read-only list result.

The receipt is a host-gate input. It is not bound to the final release commit;
the Release Decision Record remains `PENDING` until the final main commit,
annotated immutable tag, and immutable evidence archive are available. The
user's 2026-08-04 v2.0.0 decision makes cryptographic RDR/tag signatures
`NOT_REQUIRED_BY_EXPLICIT_V2_DECISION`; it does not waive the remaining gates.

## Approval confirmation

The precomputed expected values were:

| Field | Expected value | Screen comparison |
| --- | --- | --- |
| `session_id` | `pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf` | exact |
| `candidate_id` | `cand_spec_019fc864-7826-7220-a35e-d57945cf1e6a` | exact |
| `semantic_hash` | `sha256:2a6353e239f7da308d3e03a3ca7283c5cbbd317e640f6526619229585989071b` | exact |
| expected `revision` | `5` | exact |
| `base_domain_revision` | `3` | exact |
| `command_id` | `cmd_019fc864-7826-7220-a35e-d57945cf1e6a` | exact |

The Codex confirmation screen showed the following exact tool arguments before
input was submitted:

```json
{"session_id":"pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf","expected_revision":5,"candidate_id":"cand_spec_019fc864-7826-7220-a35e-d57945cf1e6a","semantic_hash":"sha256:2a6353e239f7da308d3e03a3ca7283c5cbbd317e640f6526619229585989071b","base_domain_revision":3,"command_id":"cmd_019fc864-7826-7220-a35e-d57945cf1e6a"}
```

Only after this exact comparison was Enter sent to the Codex confirmation UI.
The resulting event was successful and non-replayed:

```json
{
  "command_id": "cmd_019fc864-7826-7220-a35e-d57945cf1e6a",
  "observed": {"evidence_current": true, "projection_status": "unchanged", "warnings": []},
  "ok": true,
  "protocol_version": 1,
  "replayed": false,
  "request_id": "req_019fc86c-fd03-7e43-8702-ce40983b2a3b",
  "result": {"approval": {"approval_event_seq": 6, "base_revision": 3}}
}
```

The TUI rendered the candidate field abbreviated in one result line; the full
candidate and semantic hash were visible in the confirmation arguments above.
The subsequent `planning_status` read returned `ok: true`, `revision: 6`,
`event_count: 6`, `domain_revision: 3`, `approval_event_seq: 6`,
`evidence_current: true`, `projection_status: "unchanged"`, `warnings: []`,
and `replayed: false` (`request_id`:
`req_019fc86d-23ae-7be0-bb11-6f3611edb178`).

## Purge confirmation

The precomputed purge values were:

| Field | Expected value | Screen comparison |
| --- | --- | --- |
| `session_id` | `pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf` | exact |
| `confirm` | `pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf` | exact same session identity |
| expected `revision` | `6` | exact |
| `command_id` | `cmd_2b3b6e26-2fb5-4cd4-9ac2-9d0a70837756` | exact |

The screen showed the exact purge call:

```json
{"session_id":"pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf","expected_revision":6,"confirm":"pln_019fc864-759b-76a3-9fc8-72e9e9da5bbf","command_id":"cmd_2b3b6e26-2fb5-4cd4-9ac2-9d0a70837756"}
```

Only after the session identity, revision, command ID, and exact argv matched
was Enter sent. The resulting purge event was:

```json
{
  "command_id": "cmd_2b3b6e26-2fb5-4cd4-9ac2-9d0a70837756",
  "observed": {"evidence_current": null, "projection_status": "unchanged", "warnings": []},
  "ok": true,
  "protocol_version": 1,
  "replayed": false,
  "request_id": "req_019fc86e-34f4-7261-9ac6-1840f72fb0a6",
  "result": {"cleanup_state": "clean", "operation": "planning.purge", "purged": true, "schema": "megara.result/v1"}
}
```

The final read-only `planning_list` returned `ok: true` with
`projection_status: "unchanged"`, `warnings: []`, and `sessions: []`
(`request_id`: `req_019fc86e-459c-7591-b54c-9d7ac9f6e059`). No additional
mutation was submitted. The Codex TUI was bounded to the observed output and
then closed with EOF; it was not left waiting indefinitely.

## Gate result

`MPC-MAN-005` is satisfied for this isolated Codex host fixture: the approval
and purge mutations occurred only after exact host confirmation, and the
before/after event evidence is recorded above. Final release acceptance remains
pending the plain-text approved RDR decision, final main integration, annotated
immutable unsigned tag, and tag workflow archive.
