---
type: Plan
title: Megara Planning Core v1 운영·마이그레이션·릴리스 절차
description: Planning Core v1의 migration, rollback, doctor repair, purge residue와 release evidence 운영 절차.
timestamp: 2026-08-03
tags: [okf, plan, planning, migration, rollback, release, operations]
---

# Megara Planning Core v1 운영·마이그레이션·릴리스 절차

이 문서는 구현 계약의 요약이 아니라 운영자가 실제 명령과 안전 경계를 확인하기 위한 절차다. 상태의 canonical source는 project-local `.megara/planning/planning.db`와 event log이며, Codex와 Pi projection은 복구 가능한 출력물이다.

## Migration과 rollback

기존 project를 변경하기 전에는 항상 dry-run 결과를 보관한다.

```bash
megara planning migrate --dry-run --json > migration-dry-run.json
megara planning migrate --apply --json > migration-apply.json
```

apply는 source 파일을 backup manifest와 함께 `.megara/migration-backups/<migration-id>/`에 보존한 뒤에만 managed legacy 파일을 제거한다. legacy 의미를 자동으로 spec·plan 승인으로 바꾸지 않으며, imported session은 다시 audit와 사용자 승인을 거친다.

중단된 apply는 journal의 migration ID와 마지막 완료 단계로 식별한다.

```bash
megara planning migrate --resume <migration-id> --json
megara planning migrate --rollback <migration-id> --json
megara planning migrate --rollback <migration-id> --force --json
```

rollback은 manifest hash와 source/destination digest를 확인한다. migration이 만든 session이 이후 변경되지 않았을 때만 purge하고, 변경된 session은 `ROLLBACK_CONFLICT`로 중단한다. `--force` rollback은 purge 전에 export를 남긴다. backup manifest가 없거나 검증되지 않으면 삭제·복원을 추측해서 진행하지 않는다.

## Doctor와 purge 복구

기본 doctor는 read-only다. JSON report의 `PROJECTION_DIVERGED`, `PROJECTION_MISSING`, `PROJECTION_STALE`, `PURGE_RESIDUE`, `TOMBSTONE_INVALID`, `DB_CORRUPT` warning과 observations를 release evidence에 보관한다.

```bash
megara doctor --scope project --target codex --json
megara doctor --scope project --target codex --repair --json
```

`--repair`가 허용하는 변경은 event를 다시 쓰지 않는 replay cache 복구, managed Markdown projection/manifest 재생성, purge artifact·linked migration backup·storage cleanup retry뿐이다. repair 전후 event ID와 `state_hash_after`가 달라지면 해당 결과는 실패로 판정한다. invalid tombstone과 SQLite corruption은 자동 보정하지 않고 warning과 수동 복구 blocker로 남긴다.

purge는 먼저 SQLite transaction에서 session, event, command result를 논리적으로 제거하고 최소 tombstone을 남긴다. artifact directory, linked backup, WAL checkpoint와 VACUUM은 commit 뒤 cleanup 단계다. 이 단계가 실패하면 `cleanup_state=pending`과 필요한 `pending_backup_id`를 보존하며, `doctor --repair`가 재시도한다. partial redaction이나 hidden selection deletion은 제공하지 않는다.

`secure_delete`, DELETE, WAL truncate와 VACUUM은 app-level 범위의 잔여를 줄이지만 SSD wear-leveling, 외부 backup, filesystem snapshot까지 포함하는 forensic erase를 보장하지 않는다.

## Release evidence

release tag의 commit과 clean worktree를 고정한 뒤 evidence bundle을 만든다.

```bash
RELEASE_COMMIT="$(git rev-parse HEAD)" \
  MEGARA_EVIDENCE_ROOT="target/megara-evidence/$(git rev-parse HEAD)" \
  ./scripts/release-evidence.sh
```

스크립트는 `cargo fmt --check`, `cargo check --all-targets --locked`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked`, docs check, installer shell syntax, diff check와 DB/FS/protocol/migration/purge trace를 실행한다. `target/megara-evidence/<release-commit>/manifest.json`에는 release commit, `Cargo.lock` hash, toolchain, platform, command exit code와 manifest 자신을 제외한 모든 archive 파일의 SHA-256이 기록된다.

GitHub Release workflow는 이 staging directory를 authoritative source로 취급하지 않고 다음 immutable artifact로 업로드한다.

```text
megara-planning-release-evidence-<release-commit>
retention: 90 days
```

빈 archive는 허용하지 않으며(`if-no-files-found: error`), release commit 이후 source 또는 `Cargo.lock`이 바뀌면 기존 bundle은 무효다. raw stdout/stderr, test report와 trace를 삭제하거나 요약본으로 대체하지 않는다.

v1 release claim은 macOS arm64와 Linux x86_64에 한정한다. 각 target의 automated install, update, doctor, SQLite WAL, path protection, migration과 purge 결과를 보관하고, 실제 Codex/Pi host confirmation과 screen/command evidence는 별도 manual gate로 검토한다. 기본 release 정책은 항목별 reviewer sign-off와 cryptographically signed RDR/tag를 요구한다. 단, 2026-08-04 사용자의 명시적 결정으로 `v2.0.0`만 plain-text approved decision과 cryptographically unsigned annotated tag를 사용한다. 이 예외는 서명 검증을 통과한 것으로 표시하지 않으며 미래 release의 기본 정책을 바꾸지 않는다. v2.0.0에서도 exact final main target, annotated tag object, immutable ref, evidence hash와 모든 나머지 acceptance gate는 그대로 필수다. 현재 Pi version 계약은 `>=0.80.10` 및 `<0.81.0`이며, host가 이 범위를 벗어나면 skip/waive하지 않고 release blocker로 기록한다.

## Release Decision Record

최종 release는 [Planning Core v1 개발 완료 판정 체크리스트](planning-core-redesign-completion-checklist.md)의 Release Decision Record에 다음을 연결한 뒤에만 PASS다. v2.0.0은 아래의 일반 signed RDR/tag 정책에 대한 명시적이고 release-scoped한 예외를 사용하며, 예외 항목은 `NOT_REQUIRED_BY_EXPLICIT_V2_DECISION`으로 기록한다.

- 동일 release commit, clean tree, `Cargo.lock` hash와 toolchain
- `VB-RELEASE` raw command logs와 evidence manifest hash
- migration/rollback, purge/tombstone, DB/FS/protocol trace
- Codex/Pi host matrix와 항목별 reviewer decision
- 모든 BLOCKING·MANUAL check의 PASS 또는 명시적 external blocker
- v2.0.0의 plain-text unsigned decision과 annotated tag target/immutability receipt

BLOCKING 또는 MANUAL 항목이 미검증이면 issue를 닫거나 release PASS로 표시하지 않는다. v2.0.0의 두 cryptographic signature gate는 사용자의 dated decision으로만 `NOT_REQUIRED_BY_EXPLICIT_V2_DECISION`이 될 수 있으며, 다른 gate를 완화하지 않는다. 이 문서와 채팅은 spec/evidence가 될 수 있지만 execution tracker는 GitHub Issues의 #11 umbrella와 active Slice issue #4다.
