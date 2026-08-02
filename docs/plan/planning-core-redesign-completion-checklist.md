---
type: Plan
title: Megara Planning Core v1 개발 완료 판정 체크리스트
description: Planning Core v1 구현 완료를 증거, 실패 신호, negative proof, E2E와 수동 검토로 판정하기 위한 release gate.
timestamp: 2026-08-02
tags: [okf, plan, planning, megara, checklist, acceptance]
---

# Megara Planning Core v1 개발 완료 판정 체크리스트

> **판정 기준 문서:** [Megara Planning Core v1 기획안](planning-core-redesign.md) 2026-08-02 canonical 원문. 이 체크리스트는 해당 원문만을 완료 판정의 근거로 사용한다.

## 1. 판정 규칙 및 증거 묶음

### 1.1 최종 판정

최종 판정은 `PASS` 또는 `FAIL`만 사용한다. 조건부 통과, 부분 통과, 구현 완료 추정은 허용하지 않는다.

#### PASS

다음을 모두 만족할 때만 `PASS`다.

1. 모든 `BLOCKING` 항목이 `PASS`다.
2. 모든 `MANUAL` 항목이 서명된 검토 기록과 함께 `PASS`다.
3. 모든 `TO_DEFINE` 항목이 관련 구현과 검증을 완료 처리하기 전에 확정되어 체크리스트의 검증 가능한 계약으로 대체됐다.
4. 기준선에서 통과했던 검증에 신규 실패가 없다.
5. planning 관련 신규 테스트에는 실패, ignored, skipped, 미실행 항목이 없다.
6. 필수 E2E 시나리오가 동일 release commit을 대상으로 통과했다.
7. 증거 묶음이 동일한 release commit, `Cargo.lock`, toolchain을 가리킨다.
8. negative-proof 항목이 정적 검색, 설치 결과, runtime 동작 세 관점에서 모두 통과했다.
9. Release Decision Record가 완성되고 release authority가 서명했다.

#### FAIL

다음 중 하나라도 해당하면 `FAIL`이다.

- `BLOCKING` 항목이 실패하거나 미검증이다.
- `MANUAL` 항목이 실패하거나 검토자 서명이 없다.
- `TO_DEFINE` 항목이 미확정이다.
- 테스트 명령은 성공했지만 실행된 테스트 수가 0이다.
- 관련 테스트가 ignored 또는 skipped 상태다.
- baseline failure라는 주장을 base commit에서 재현하지 못한다.
- planning 변경 이후 실패 수, diagnostic 종류 또는 실패 범위가 증가했다.
- grep 결과만으로 runtime 삭제를 주장한다.
- 수동 검토가 “검토함”, “문제없음” 같은 요약만 남기고 항목별 판단을 남기지 않았다.
- 로그가 동일 release commit에서 생성됐다는 증거가 없다.
- 실패한 실행 로그가 누락되고 성공한 재실행 결과만 제출됐다.
- release 대상 작업 트리에 증거에 포함되지 않은 변경이 있다.
- 증거 파일이 수정됐거나 원본 hash를 확인할 수 없다.

### 1.2 항목 등급

| 등급 | 의미 | 최종 판정 영향 |
| --- | --- | --- |
| `BLOCKING` | 코드, 저장소, 프로토콜, 보안, migration 또는 회귀에 대한 필수 계약 | 실패·미검증·허용되지 않은 N/A이면 전체 `FAIL` |
| `MANUAL` | 자연어 품질, host confirmation, 사용자 복구 경험처럼 사람의 직접 판정이 필요한 필수 계약 | 실패·미서명·증거 누락이면 전체 `FAIL` |
| `INFORMATIONAL` | 위협 모델 밖의 한계, 알려진 baseline failure, 비차단 관찰 사항 | 단독으로 release를 차단하지 않지만 Release Decision Record에 남겨야 함 |

### 1.3 검증 방식

| 방식 | 정의 |
| --- | --- |
| `자동` | 명령, 테스트, fixture 비교, DB 조회, 파일 hash 또는 protocol assertion으로 재현 가능 |
| `수동` | 검토자가 실제 UI·문서·복구 흐름을 확인하고 항목별 판정과 서명을 남김 |
| `혼합` | 자동 증거와 수동 확인이 모두 필요 |

### 1.4 항목 상태

각 체크는 다음 상태 중 하나만 가진다.

- `PASS`
- `FAIL`
- `UNVERIFIED`
- `NOT_APPLICABLE`

`BLOCKING`과 `MANUAL` 항목에 `NOT_APPLICABLE`을 사용하려면 해당 체크 자체가 조건부라고 명시되어 있어야 하며, 조건이 발생하지 않았다는 자동 증거가 필요하다. 그 외 `NOT_APPLICABLE`은 `FAIL`로 판정한다.

### 1.5 허용되는 증거

“구현됨”, “동작함”, “검토함”, “테스트 추가함”은 증거가 아니다.

각 자동 증거에는 최소한 다음이 있어야 한다.

- release commit SHA
- 작업 트리 clean 여부
- `Cargo.lock` SHA-256
- `rustc -Vv`
- `cargo -V`
- 실행 명령
- 시작·종료 시각
- exit code
- stdout 원문
- stderr 원문
- 실행된 테스트 수
- 실패·ignored·skipped 테스트 수
- 생성되거나 비교된 fixture·DB·파일의 hash
- 증거 원문 자체의 SHA-256

각 수동 증거에는 최소한 다음이 있어야 한다.

- 체크 ID
- release commit SHA
- 검토 환경
- 검토자 이름과 역할
- 검토 일자
- 확인한 화면·로그·파일
- 항목별 `예/아니오`
- 실패 시 재현 절차
- 최종 서명

### 1.6 증거 조작 방지 규칙

1. test 이름이 존재하는 것만으로 통과하지 않는다. 해당 테스트가 실제 assertion을 실행해야 한다.
2. 필터 때문에 테스트가 0개 실행되면 해당 검증은 실패다.
3. `#[ignore]`, 조건부 skip, 환경 미충족으로 실행되지 않은 필수 테스트는 실패다.
4. fixture가 기대 결과를 자기 자신으로 다시 생성하면 golden 검증으로 인정하지 않는다.
5. negative grep은 삭제 증거의 일부일 뿐이다. 설치 결과와 runtime 거부도 확인해야 한다.
6. manual review는 항목별 판단 없이 한 줄 서명만 있으면 실패다.
7. baseline과 release 검증은 서로 다른 checkout을 사용하되, 각각의 commit과 toolchain을 기록한다.
8. 실패 후 재실행했다면 최초 실패와 최종 성공 로그를 모두 보존한다.
9. release 증거 생성 후 코드나 `Cargo.lock`이 바뀌면 모든 증거는 무효다.
10. generated Markdown만 비교하고 DB·event·normalized state를 확인하지 않은 테스트는 authoritative state 검증으로 인정하지 않는다.

### 1.7 Baseline failure와 신규 failure 구분

#### Baseline failure 인정 조건

다음을 모두 만족해야 한다.

- planning patch가 없는 canonical base commit에서 재현된다.
- 동일 `Cargo.lock`과 동일 Rust toolchain을 사용한다.
- 동일한 테스트 이름 또는 동일한 rustc/clippy error code가 나온다.
- 경로와 line number를 제외한 diagnostic 핵심 문구가 같다.
- release 변경으로 실패 수가 증가하지 않았다.
- planning 관련 테스트가 아니다.
- base commit 실행 원문과 release commit 실행 원문이 모두 보존됐다.

#### 신규 failure

다음 중 하나면 신규 failure다.

- baseline에서 성공한 명령이 release commit에서 실패했다.
- 신규 planning 테스트가 실패했다.
- 기존 실패 수가 증가했다.
- 새로운 rustc 또는 clippy diagnostic이 추가됐다.
- adapter, migration, fixture, golden, docs 검증이 실패했다.
- baseline failure와 동일하다고 주장한 diagnostic이 base commit에서 재현되지 않는다.
- planning 테스트를 baseline allow-list로 제외했다.

신규 failure가 하나라도 있으면 최종 판정은 `FAIL`이다.

### 1.8 공통 Verification Bundle

같은 명령을 개별 체크마다 반복하지 않고 다음 묶음을 참조한다.

#### `VB-BASELINE`

~~~
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --quiet -- docs check --root docs
~~~

필수 부가 증거:

- base commit SHA
- `Cargo.lock` hash
- toolchain
- 실패 목록과 normalized diagnostic
- 기존 install, hook, Pi projection snapshot
- tracked legacy path fixture

#### `VB-DOMAIN`

~~~
cargo test --test unit planning_domain
cargo test --test unit planning_engine
~~~

#### `VB-STORE-PROTOCOL`

~~~
cargo test --test unit planning_store
cargo test --test unit planning_protocol
cargo test --test integration planning_cli
~~~

#### `VB-EVIDENCE-INTERVIEW`

~~~
cargo test --test integration planning_cli
cargo test --test unit planning_engine
cargo test --test unit planning_protocol
~~~

#### `VB-ARTIFACT`

~~~
cargo test --test unit planning_artifacts
cargo test --test integration planning_cli
~~~

#### `VB-ADAPTER`

~~~
cargo test --test integration planning_mcp
cargo test --test integration planning_pi
cargo test --test integration planning_install
cargo test --test integration planning_adapter_equivalence
cargo test --all-targets
~~~

#### `VB-MIGRATION-NEGATIVE`

~~~
cargo test --test integration planning_migration
cargo test --test integration planning_install
cargo test --all-targets
rg -n 'deep-interview|ralplan|ultragoal|SubagentStart|SubagentStop|PreToolUse|PostToolUse' Cargo.toml Cargo.lock harness src test docs
~~~

허용되는 `rg` 결과는 다음 범위로 제한한다.

- `src/planning/migration.rs`
- `test/fixtures/planning/legacy/`
- 비교·migration·history 목적의 문서

그 밖의 `src`, `harness`, Cargo dependency, installer runtime projection 결과는 0개여야 한다.

#### `VB-RELEASE`

~~~
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --quiet -- docs check --root docs
git diff --check
~~~

### 1.9 공통 증거 유형

| 증거 ID | 내용 |
| --- | --- |
| `ER-CMD` | 원문 command log, exit code, 환경 metadata |
| `ER-TEST` | 테스트 이름, 실행 수, assertion 결과, ignored/skipped 수 |
| `ER-DB` | SQLite schema·PRAGMA·row·hash·replay 비교 결과 |
| `ER-FS` | 설치 전후 파일 tree, file hash, permission, backup 비교 |
| `ER-PROTOCOL` | request·response 원문, framing, typed error, stdout/stderr 분리 |
| `ER-HOST` | Codex/Pi 실제 host confirmation·화면·tool invocation 기록 |
| `ER-MIGRATION` | manifest 단계, source/destination hash, crash point, resume·rollback 기록 |
| `ER-PURGE` | 삭제 전후 DB/WAL/artifact/backup byte scan과 tombstone |
| `ER-MANUAL` | 검토자별 항목 판정과 서명 |
| `ER-TRACE` | 계획 section, requirement, test, evidence 간 추적 manifest |

### 1.10 Release gate 실행 요약

아래 체크박스는 상세 표의 대체물이 아니다. 각 항목은 대응 check ID와 evidence hash가 Release Decision Record에 연결됐을 때만 표시한다.

- [ ] release commit, clean tree, `Cargo.lock`, toolchain과 대상 플랫폼을 고정했다.
- [ ] `MPC-TDF-*`를 모두 승인된 계약과 실행된 검증으로 대체했다.
- [ ] `VB-BASELINE`부터 `VB-RELEASE`까지 같은 release commit 기준으로 실행했다.
- [ ] 모든 `BLOCKING` check가 `PASS`이고 ignored·skipped·filtered-out 필수 테스트가 없다.
- [ ] 모든 `MANUAL` check가 항목별 근거와 검토자 서명을 가진다.
- [ ] `MPC-NEG-*`가 source, install, runtime 관점에서 요구된 증거를 모두 가진다.
- [ ] `MPC-E2E-001`부터 `MPC-E2E-020`까지 독립 fixture에서 통과했다.
- [ ] baseline과 비교해 신규 failure가 0개다.
- [ ] 증거 archive manifest와 각 원문 증거의 hash를 검증했다.
- [ ] Release Decision Record가 모든 check ID를 추적하고 release authority가 `PASS`로 서명했다.

---

## 2. 계획 section·slice → 체크 ID 추적표

### 2.1 계획 section 추적

| 계획 section | 핵심 계약 | 체크 ID |
| --- | --- | --- |
| §0–§4 | 제품 지위, 확정 결정, 저장소 경계, 목표·비목표 | `MPC-SCP-*`, `MPC-REG-*`, `MPC-NEG-*` |
| §5 | PlanningCore·모델·adapter·projection 소유권 | `MPC-OWN-*`, `MPC-ADP-*` |
| §6 | project root, 저장 구조, Git ignore | `MPC-SCP-003`, `MPC-REG-003`, `MPC-REG-004` |
| §7 | SQLite, transaction, replay, command result | `MPC-DB-*`, `MPC-EVT-*`, `MPC-PUR-*` |
| §8 | lifecycle, orthogonal state, revision, transition | `MPC-STM-*`, `MPC-AUD-*` |
| §9 | entity, edge, provenance, validity, blocker | `MPC-STM-009`–`MPC-STM-013`, `MPC-AUD-004` |
| §10 | aggregate event와 모델 비결정성 | `MPC-EVT-*`, `MPC-OWN-004` |
| §11 | canonicalization, hash, invalidation | `MPC-SPC-004`, `MPC-PLN-005`, `MPC-STM-007` |
| §12 | repo evidence와 freshness | `MPC-EVD-*` |
| §13 | work item, QuestionProposal, audit, wire schema | `MPC-QST-*`, `MPC-AUD-*`, `MPC-PRO-*` |
| §14 | spec candidate와 승인 | `MPC-SPC-*`, `MPC-ADP-008` |
| §15 | plan candidate와 구조 validator | `MPC-PLN-*` |
| §16–§17 | CLI, session, envelope, idempotency, errors | `MPC-CLI-*`, `MPC-PRO-*` |
| §18 | Codex MCP, Pi RPC, QuestionProjection, adapter equivalence | `MPC-ADP-*`, `MPC-MCP-*`, `MPC-PI-*` |
| §19 | projection, export, purge | `MPC-PRJ-*`, `MPC-EXP-*`, `MPC-PUR-*` |
| §20 | migration, journal, rollback, legacy 제거 | `MPC-MIG-*`, `MPC-NEG-*` |
| §21 | 실제 파일 책임 경계 | `MPC-OWN-001`, `MPC-ADP-003`, `MPC-REG-007` |
| §24 | baseline failure 구분 | `MPC-BAS-*` |
| §26 | 보증 문구 | `MPC-SCP-008`, `MPC-MAN-008` |
| §27 | 후속 개선 범위 | `MPC-INF-001`–`MPC-INF-003` |

### 2.2 §22 구현 slice 추적

| Slice | 체크 ID | Verification bundle | 완료 판정 핵심 |
| --- | --- | --- | --- |
| Slice 0 — Baseline과 inventory | `MPC-BAS-001`–`MPC-BAS-006`, `MPC-NEG-001` | `VB-BASELINE` | baseline과 legacy inventory가 재현 가능하며 runtime 변화가 없음 |
| Slice 1 — Domain과 in-memory engine | `MPC-OWN-*`, `MPC-STM-*` | `VB-DOMAIN` | 모델·filesystem 없이 invariant와 transition 검증 |
| Slice 2 — SQLite, replay, logical protocol | `MPC-DB-*`, `MPC-EVT-*`, `MPC-CLI-*`, `MPC-PRO-*`, `MPC-PUR-001`–`004` | `VB-STORE-PROTOCOL` | atomic event/state, replay, idempotency, RPC |
| Slice 3 — Evidence와 interview loop | `MPC-EVD-*`, `MPC-QST-*`, `MPC-AUD-*` | `VB-EVIDENCE-INTERVIEW` | evidence freshness, one-question, full audit, novice 질문 계약 |
| Slice 4 — Spec과 plan artifact | `MPC-SPC-*`, `MPC-PLN-*`, `MPC-PRJ-*`, `MPC-EXP-*` | `VB-ARTIFACT` | exact approval binding, traceability, projection conflict |
| Slice 5 — Codex MCP와 Pi adapter | `MPC-ADP-*`, `MPC-MCP-*`, `MPC-PI-*` | `VB-ADAPTER` | MCP/Pi 경계, host confirmation, adapter equivalence |
| Slice 6 — Migration과 legacy runtime 제거 | `MPC-MIG-*`, `MPC-NEG-*` | `VB-MIGRATION-NEGATIVE` | crash-resumable migration, rollback, 실제 legacy 제거 |
| Slice 7 — Doctor와 release hardening | `MPC-REG-*`, `MPC-PUR-009`–`012`, `MPC-TST-*` | `VB-RELEASE` | read-only doctor, safe repair, residue 처리, 전체 release 증거 |

### 2.3 §23 Test matrix 추적

| §23 영역 | 체크 ID |
| --- | --- |
| Session | `MPC-CLI-004`–`MPC-CLI-006`, `MPC-E2E-004` |
| Question | `MPC-QST-001`–`MPC-QST-012` |
| Question schema | `MPC-QST-003`–`MPC-QST-007` |
| Question invalidation | `MPC-QST-008`, `MPC-QST-009` |
| Question authoring | `MPC-QST-001`, `MPC-QST-010`, `MPC-MAN-001` |
| Question projection | `MPC-QST-011`, `MPC-QST-012`, `MPC-ADP-012` |
| Revision | `MPC-STM-004`, `MPC-DB-007`, `MPC-PRO-006` |
| Idempotency | `MPC-DB-006`, `MPC-PRO-005`, `MPC-PUR-005` |
| Phase | `MPC-STM-001`–`MPC-STM-006` |
| Orthogonal state | `MPC-STM-002`, `MPC-STM-003` |
| Entity | `MPC-STM-009`–`MPC-STM-011` |
| Edge | `MPC-STM-012`, `MPC-STM-013` |
| Audit | `MPC-AUD-001`–`MPC-AUD-008` |
| Readiness | `MPC-AUD-006` |
| Model error | `MPC-PRO-009`–`MPC-PRO-012`, `MPC-RES-001` |
| Evidence | `MPC-EVD-001`–`MPC-EVD-006` |
| Evidence stale | `MPC-EVD-007`–`MPC-EVD-011` |
| Evidence security | `MPC-EVD-004`–`MPC-EVD-006`, `MPC-TDF-005` |
| Hash | `MPC-SPC-004`, `MPC-PLN-005`, `MPC-EVT-008` |
| Spec | `MPC-SPC-*` |
| Plan | `MPC-PLN-*` |
| Replay | `MPC-EVT-004`–`MPC-EVT-006` |
| Event boundary | `MPC-EVT-001`–`MPC-EVT-003` |
| Crash | `MPC-DB-004`, `MPC-PRJ-003`, `MPC-MIG-007` |
| Observed health | `MPC-STM-008`, `MPC-EVD-007` |
| RPC | `MPC-PRO-007`–`MPC-PRO-012`, `MPC-PI-001` |
| Approval entrypoint | `MPC-SPC-007`, `MPC-PLN-008`, `MPC-ADP-008`–`MPC-ADP-010` |
| MCP | `MPC-MCP-*` |
| Adapter | `MPC-ADP-011`–`MPC-ADP-014` |
| Migration | `MPC-MIG-*` |
| Purge | `MPC-PUR-*` |
| Gitignore | `MPC-REG-003`, `MPC-REG-004` |
| Scope | `MPC-SCP-*`, `MPC-NEG-*` |

### 2.4 §25 최종 수용 기준 추적

| §25 범주 | 체크 ID |
| --- | --- |
| Product | `MPC-SCP-*`, `MPC-NEG-*` |
| State | `MPC-OWN-*`, `MPC-STM-*`, `MPC-EVT-*` |
| Storage | `MPC-DB-*`, `MPC-EVT-004`–`006`, `MPC-PRJ-004` |
| Session과 protocol | `MPC-CLI-*`, `MPC-PRO-*`, `MPC-PUR-005`–`007` |
| Interview | `MPC-QST-*`, `MPC-AUD-*`, `MPC-MAN-001` |
| Approval | `MPC-SPC-006`–`009`, `MPC-PLN-007`–`009`, `MPC-ADP-008`–`010` |
| Evidence | `MPC-EVD-*` |
| Artifact | `MPC-PRJ-*`, `MPC-EXP-*` |
| Migration | `MPC-MIG-*`, `MPC-NEG-*` |
| Scope | `MPC-SCP-*`, `MPC-NEG-*`, `MPC-REG-008` |

---

## 3. 영역별 상세 체크리스트

### 3.1 제품 범위와 비목표

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-SCP-001` | BLOCKING | 혼합 | Planning flow가 spec·plan 승인에서 끝나고 구현 실행으로 전환하지 않는다. | `VB-RELEASE`, CLI/MCP/Pi E2E event·filesystem diff | `Complete` 이후 구현·테스트·commit·branch·PR·release command가 실행되지 않음 | planning 완료가 실행 workflow나 후속 agent 실행을 시작함 |
| `MPC-SCP-002` | BLOCKING | 자동 | planning 명령이 source file, Git index, commit, branch를 변경하지 않는다. | clean fixture의 command 전후 `ER-FS`, Git status 비교 | 허용된 `.megara/planning`, 명시적 export, managed install projection 외 변경 0 | source, index, branch, commit 또는 임의 project file 변경 |
| `MPC-SCP-003` | BLOCKING | 자동 | `--project`가 있으면 해당 canonical path, 없으면 cwd만 사용하고 상위 root를 탐색하지 않는다. | `planning_cli` fixture: 중첩 directory·상위 Git/.megara | 상태가 지정 root/cwd 아래에만 생성됨 | 상위 repository 또는 `.megara`가 자동 선택됨 |
| `MPC-SCP-004` | BLOCKING | 자동 | 상태는 project-local 단일 `.megara/planning/planning.db`에만 저장된다. | DB 생성 위치와 open trace | 프로젝트 밖 DB나 사용자 전역 DB 없음 | global DB, home directory state 또는 session별 DB 생성 |
| `MPC-SCP-005` | BLOCKING | 혼합 | 모델 선택, routing, fallback, reasoning level을 Megara가 변경하지 않는다. | Codex/Pi adapter source review, host logs | adapter가 현재 host model을 사용하고 설정 변경 호출 0 | 모델 ID·thinking level·fallback 변경 |
| `MPC-SCP-006` | BLOCKING | 자동 | planning TUI, daemon, queue, polling, auth, issue broker, worktree manager, dashboard를 추가하지 않는다. | `VB-MIGRATION-NEGATIVE`, dependency·module inventory | 관련 runtime/module/dependency 0 | 하나라도 planning runtime에 존재 |
| `MPC-SCP-007` | BLOCKING | 자동 | planning lifecycle이 Skill, hook, hidden metadata에 의존하지 않는다. | clean install tree, runtime tests, negative proof | Skill/hook 없이 CLI·MCP·Pi E2E가 완료됨 | Skill/hook 부재 시 workflow 진행 불가 |
| `MPC-SCP-008` | MANUAL | 수동 | 사용자 문서와 CLI가 구조적 보증과 의미적 품질을 구분한다. | 문서·help·오류 문구 review record | 승인된 보증 문구와 동등하며 “품질 기계 보증” 주장이 없음 | 의미 정확성·기술 타당성을 Rust가 보증한다고 표현 |
| `MPC-SCP-009` | BLOCKING | 자동 | 비-workflow utility가 남더라도 planning state·projector·transition에 연결되지 않는다. | dependency graph와 runtime fixture | utility 호출 없이 planning 완료, utility가 state write 불가 | utility가 hidden planning transition 수행 |

### 3.2 PlanningCore 단일 상태 소유권

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-OWN-001` | BLOCKING | 혼합 | authoritative DB write는 PlanningCore store 경계를 통해서만 수행된다. | source dependency review, write tracing, `VB-DOMAIN` | adapter·projection·UI가 store mutation API에 접근하지 않음 | adapter 또는 renderer가 DB/event를 직접 수정 |
| `MPC-OWN-002` | BLOCKING | 자동 | Codex/Pi adapter가 `rusqlite`를 import하거나 `.megara/planning`을 직접 읽고 쓰지 않는다. | source scan, adapter tests | import·path access 0 | adapter에 DB/path 직접 접근 존재 |
| `MPC-OWN-003` | BLOCKING | 자동 | 모델 출력은 versioned typed proposal로만 core에 들어간다. | invalid free-form fixture, protocol tests | schema 없는 모델 문장은 state 변화 0 | 자연어 문구로 phase/entity/approval 변경 |
| `MPC-OWN-004` | BLOCKING | 자동 | 모델은 phase, approval, canonical ID, actor를 payload로 지정할 수 없다. | unknown/forbidden field fixtures | 전체 command가 `PROPOSAL_SCHEMA_INVALID` 또는 `INVALID_REQUEST`로 거부 | 금지 field가 무시되거나 일부 적용 |
| `MPC-OWN-005` | BLOCKING | 자동 | adapter는 actor를 entrypoint에서 결정하고 request의 actor field를 받지 않는다. | RPC/MCP malformed request | actor 주입이 거부되고 event metadata는 entrypoint 값 | request actor가 승인 권한에 반영 |
| `MPC-OWN-006` | BLOCKING | 자동 | projection은 canonical input을 변경하지 않는 pure output이다. | projection edit·delete fixture | edit/delete 후 DB state/hash 불변 | Markdown 변경이 entity/approval로 역수입 |
| `MPC-OWN-007` | BLOCKING | 자동 | `PlanningCore`는 모델, Git, filesystem transport를 모르는 동기 domain/service 경계로 유지된다. | module dependency test·source review | engine/store test가 model·adapter 없이 실행 | engine이 host API나 MCP/Pi type에 의존 |

### 3.3 Lifecycle, 직교 상태, event aggregate, replay, invalidation

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-STM-001` | BLOCKING | 자동 | lifecycle enum은 `Interview`, `Specification`, `Planning`, `Complete`만 포함한다. | `VB-DOMAIN` | waiting·blocked·approved·stale가 phase variant가 아님 | 직교 상태가 phase에 혼합됨 |
| `MPC-STM-002` | BLOCKING | 자동 | waiting, blocker, approval, stale 조합이 phase와 독립적으로 계산된다. | state combination table tests | 가능한 조합별 derived 값이 계약과 일치 | phase 변경으로만 waiting/stale를 표현 |
| `MPC-STM-003` | BLOCKING | 자동 | `pending_question`과 `required_model_action`은 동시에 존재하지 않는다. | property/table-driven tests | 모든 mutation 후 둘 중 최대 하나 | 둘 다 존재하거나 next action 0개·2개 이상 |
| `MPC-STM-004` | BLOCKING | 자동 | 성공 mutation마다 `revision`이 정확히 1 증가하고 query/replay는 증가시키지 않는다. | event/state before-after | mutation 1회당 +1, query +0 | revision skip, 중복 증가, query 증가 |
| `MPC-STM-005` | BLOCKING | 자동 | answer는 pending `question_id`, pending `based_on_revision`, command `expected_revision`이 모두 일치해야 한다. | mismatch fixture | 불일치 시 event/state 변화 0 | 잘못된 질문 또는 stale answer 적용 |
| `MPC-STM-006` | BLOCKING | 자동 | 계획 §8.4의 모든 legal/illegal transition이 테스트된다. | transition matrix ↔ test trace manifest | 각 표 행과 금지 전환에 assertion 존재 | 테스트되지 않은 전환 또는 illegal transition 성공 |
| `MPC-STM-007` | BLOCKING | 자동 | domain revision 증가가 full audit, spec·plan candidate와 approval을 stale/revoke한다. | answer/entity/evidence mutation fixture | 영향을 받는 artifact 전부 stale | stale artifact가 current/approved로 남음 |
| `MPC-STM-008` | BLOCKING | 자동 | `ObservedHealth` 변화가 event, revision, normalized state hash를 바꾸지 않는다. | repo mutation 전후 read query | warning만 바뀌고 canonical state 동일 | query가 event append 또는 hash 변경 |
| `MPC-STM-009` | BLOCKING | 자동 | entity revise는 새 revision과 `supersedes`를 만들고 in-place overwrite하지 않는다. | entity history fixture | rev1 보존, rev2 current | 과거 body 덮어쓰기 |
| `MPC-STM-010` | BLOCKING | 자동 | stale entity는 새 valid revision으로만 회복되며 in-place clear가 없다. | evidence stale→revise fixture | stale revision 보존, 새 revision valid | stale flag가 기존 revision에서 제거 |
| `MPC-STM-011` | BLOCKING | 자동 | reject는 새 rejected revision을 만들고 hard delete하지 않는다. | reject replay fixture | history와 source_refs 보존 | entity row/history 삭제 |
| `MPC-STM-012` | BLOCKING | 자동 | edge 종류와 방향이 §9.2에 고정되고 dangling endpoint·중복 current edge가 거부된다. | edge schema fixtures | 잘못된 방향 전체 거부, 부분 적용 0 | 금지 방향 허용 또는 dangling edge 생성 |
| `MPC-STM-013` | BLOCKING | 자동 | stale 전파는 `derived_from`·`depends_on` downstream에 적용되고 edge 자체는 보존된다. | transitive graph fixture | 영향 node stale, unrelated node valid | 누락 invalidation 또는 무관 node stale |
| `MPC-EVT-001` | BLOCKING | 자동 | 성공 mutation 하나는 aggregate semantic event 하나만 append한다. | DB event count before-after | event count +1 | +0 또는 +2 이상 |
| `MPC-EVT-002` | BLOCKING | 자동 | `seq == revision_after`이며 1부터 연속이다. | replay fixture·DB query | gap·duplicate 없음 | gap, duplicate, 불일치 |
| `MPC-EVT-003` | BLOCKING | 자동 | reducer가 primary와 effects를 재검산하고 저장된 revision/hash를 맹신하지 않는다. | tampered event fixture | 불일치를 corruption으로 판정 | 조작된 effects/revision을 그대로 수용 |
| `MPC-EVT-004` | BLOCKING | 자동 | replay는 모델·network·Git·filesystem·Markdown을 호출하지 않는 pure fold다. | I/O trap/mocking test | 외부 호출 0 | replay 중 외부 I/O 발생 |
| `MPC-EVT-005` | BLOCKING | 자동 | cache 삭제 후 event만으로 같은 canonical state와 hash를 재구성한다. | state cache 삭제 fixture | byte-normalized state/hash 일치 | 필드 누락, 다른 hash |
| `MPC-EVT-006` | BLOCKING | 자동 | cache/hash 불일치는 `PROJECTION_DIVERGED`이며 자동 event 수정이 없다. | corrupted cache fixture | doctor 진단, event row 불변 | 자동 승인·event rewrite |
| `MPC-EVT-007` | BLOCKING | 자동 | unknown event/effect/schema field는 `deny_unknown_fields`로 전체 거부된다. | forward/unknown fixture | event append 0 | unknown field 무시 또는 부분 적용 |
| `MPC-EVT-008` | BLOCKING | 자동 | normalized state hash는 metadata·UUID 차이를 제거하면서 관계를 보존한다. | 동일 semantic event/다른 UUID fixture | 같은 normalized hash | metadata 차이로 hash 불일치 또는 관계 손실 |
| `MPC-EVT-009` | BLOCKING | 자동 | 같은 input hash에서 다른 모델 proposal을 적용해도 각 결과가 별 event로 보존되고 replay 시 재호출하지 않는다. | two-proposal fixture | 두 이력 보존, 선택된 state 재현 | 이전 proposal overwrite 또는 replay 재평가 |

### 3.4 SQLite, transaction, locking, crash recovery

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-DB-001` | BLOCKING | 자동 | DB open 시 WAL, FULL synchronous, foreign keys, 5000ms busy timeout, secure delete가 적용된다. | `ER-DB`, PRAGMA query | 모든 값이 계획과 일치 | 하나라도 누락·다른 값 |
| `MPC-DB-002` | BLOCKING | 자동 | schema v1에 계획 §7.2의 최소 table·column·unique constraint가 존재한다. | schema dump/golden | schema golden 일치 | table/constraint 누락 |
| `MPC-DB-003` | BLOCKING | 자동 | `sessions.state_json`은 cache이고 events가 원본임을 replay/repair가 증명한다. | cache delete/corrupt tests | event만으로 복구 | cache 없으면 session 소실 |
| `MPC-DB-004` | BLOCKING | 자동 | mutation은 `BEGIN IMMEDIATE` transaction에서 command 검증, event, state, core result를 atomic commit한다. | crash injection: 각 write 경계 | commit 전 전부 rollback, commit 후 전부 존재 | event만 존재하거나 state/result만 존재 |
| `MPC-DB-005` | BLOCKING | 자동 | 5초 내 writer lock을 얻지 못하면 `DB_BUSY`이며 부분 변경이 없다. | lock holder fixture | typed error, event/revision +0 | 무한 대기, 다른 error, 부분 write |
| `MPC-DB-006` | BLOCKING | 자동 | 동일 command ID·동일 request hash는 저장된 core result를 replay하고 event를 추가하지 않는다. | process restart 포함 | `replayed=true`, revision/event 불변 | 중복 event 또는 다른 core result |
| `MPC-DB-007` | BLOCKING | 자동 | 동일 command ID·다른 request hash는 `COMMAND_ID_REUSE`다. | conflicting request fixture | state 변화 0 | 새 command로 처리 |
| `MPC-DB-008` | BLOCKING | 자동 | concurrent writer에서 lost update가 없고 stale writer는 revision conflict 또는 DB busy로 종료된다. | 동시 process test | 최종 revision/event가 성공 mutation 수와 일치 | 덮어쓰기, 누락, duplicate |
| `MPC-DB-009` | BLOCKING | 자동 | 낮은 schema는 `SCHEMA_UPGRADE_REQUIRED`, 높은 schema는 `SCHEMA_VERSION_UNSUPPORTED`다. | fake schema fixture | 일반 command가 자동 migration하지 않음 | 자동 변경 또는 잘못된 error |
| `MPC-DB-010` | BLOCKING | 자동 | SQLite corruption과 replay divergence를 구분해 typed error로 반환한다. | corrupted page/cache fixtures | `DB_CORRUPT` 또는 `PROJECTION_DIVERGED` 정확히 반환 | panic, silent reset, 잘못된 분류 |
| `MPC-DB-011` | BLOCKING | 자동 | doctor 기본 실행은 DB/event를 수정하지 않고 `--repair`만 cache·Markdown을 재생성한다. | DB/file hash before-after | 기본 hash 동일, repair 후 event hash 동일 | 기본 doctor write 또는 repair가 event 수정 |

### 3.5 CLI, session 선택, logical protocol, RPC/MCP framing

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-CLI-001` | BLOCKING | 자동 | §16.4의 command tree가 존재하고 별도 team/ultragoal 실행 명령이 없다. | CLI parser snapshot·integration tests | 지정 command 모두 parse, 제거 command 부재 | 필수 command 누락 또는 legacy 실행 command 잔존 |
| `MPC-CLI-002` | BLOCKING | 자동 | 모든 명령이 `--project`와 `--json` 계약을 지킨다. | command matrix test | 각 command JSON output parse 성공 | 일부 명령의 JSON 미지원·사람용 출력 혼입 |
| `MPC-CLI-003` | BLOCKING | 자동 | mutation은 start 외 `--session`과 `expected_revision`이 필수다. | missing arg fixtures | CLI/schema error, state 변화 0 | fallback mutation 또는 revision 없는 write |
| `MPC-CLI-004` | BLOCKING | 자동 | read-only fallback은 유일한 open session 또는 유일한 전체 session에서만 허용된다. | 0·1·다중 session fixture | 계획 순서와 일치 | 임의 active session 선택 |
| `MPC-CLI-005` | BLOCKING | 자동 | 다중 후보에서는 `SESSION_AMBIGUOUS`, 0개에서는 `SESSION_NOT_FOUND`다. | session matrix | typed error 일치 | 첫 session 자동 선택 |
| `MPC-CLI-006` | BLOCKING | 자동 | `list`는 session 선택을 수행하지 않는다. | multi-session fixture | 전체 필터 결과 반환 | ambiguity error 또는 임의 선택 |
| `MPC-CLI-007` | BLOCKING | 자동 | `megara define`은 start alias이며 실행 workflow를 시작하지 않는다. | alias output/event 비교 | start와 semantic state 동일 | 별도 hidden flow 또는 implementation 시작 |
| `MPC-CLI-008` | BLOCKING | 자동 | `megara plan`은 current query alias이고 LLM을 직접 호출하지 않는다. | no-network/model trap | work item 반환, 외부 호출 0 | model 호출 또는 plan 자동 적용 |
| `MPC-CLI-009` | BLOCKING | 자동 | exit code 0·2·3·5가 §16.5 분류와 일치한다. | error matrix | 입력/충돌/DB 오류가 정확한 code | typed error와 exit code 불일치 |
| `MPC-CLI-010` | BLOCKING | 자동 | CLI가 자동 생성한 UUIDv7 command ID를 JSON 응답에 포함한다. | mutation output fixture | valid command ID 반환 | 누락, 비재사용 불가능한 임의 값 |
| `MPC-PRO-001` | BLOCKING | 자동 | logical request envelope의 필수·금지 필드가 §17.1과 일치한다. | schema golden | query/mutation별 정확한 필드 | 필수 누락 허용 또는 actor 수용 |
| `MPC-PRO-002` | BLOCKING | 자동 | canonical request hash는 계획의 포함·제외 필드를 정확히 따른다. | formatting/request_id/adapter/force variation | logical 동일 request의 hash 동일 | 제외 필드 변화로 hash 변경 또는 params 변화 무시 |
| `MPC-PRO-003` | BLOCKING | 자동 | success 응답이 authoritative `result`와 재계산된 `observed`를 분리한다. | idempotent replay fixture | core result 동일, observed만 현재 상태 반영 | observed가 command result에 고정되거나 state를 변경 |
| `MPC-PRO-004` | BLOCKING | 자동 | §17.5 typed error enum이 고정되고 unknown generic error로 대체되지 않는다. | error coverage manifest | 모든 enum에 유발 fixture 존재 | 미검증 error, 문자열-only error |
| `MPC-PRO-005` | BLOCKING | 자동 | 동일 logical mutation과 command ID는 CLI·MCP·Pi transport가 달라도 같은 request hash를 사용한다. | cross-transport replay | event 1개, 이후 replay | transport별 중복 event |
| `MPC-PRO-006` | BLOCKING | 자동 | stale `expected_revision`은 `REVISION_CONFLICT`이며 state 변화가 없다. | stale client fixture | event/revision +0 | stale write 적용 |
| `MPC-PRO-007` | BLOCKING | 자동 | Pi RPC는 UTF-8 JSON 한 줄 입력, JSON 한 줄 출력 후 종료한다. | raw stdin/stdout | 정확히 1 response line | multi-line, process 잔존, 추가 stdout |
| `MPC-PRO-008` | BLOCKING | 자동 | stdout에는 protocol 외 출력이 없고 log는 stderr다. | invalid/valid request logs | stdout 전체 JSON parse 가능 | banner·log·panic text 혼입 |
| `MPC-PRO-009` | BLOCKING | 자동 | 전체 payload 4MiB 초과는 state 변화 없이 거부된다. | boundary fixture | typed input error, event +0 | allocation failure, panic, 부분 적용 |
| `MPC-PRO-010` | BLOCKING | 자동 | text 64KiB, path 4KiB, ID/temp_ref 128 byte, operation 10,000개 제한이 경계값에서 검증된다. | 이하·동일·초과 fixtures | 경계 일관, 초과 전체 거부 | off-by-one 또는 부분 적용 |
| `MPC-PRO-011` | BLOCKING | 자동 | unknown field·enum·operation은 전체 command를 거부한다. | malicious schema fixtures | event/revision +0 | unknown field 무시 |
| `MPC-PRO-012` | BLOCKING | 자동 | invalid JSON, truncated JSON, non-object JSON이 panic 없이 typed error를 반환한다. | malformed corpus | process 정상 종료/다음 요청 가능 | panic, hang, stdout 오염 |
| `MPC-PRO-013` | BLOCKING | 혼합 | operation별 request·success result schema가 golden으로 고정된다. | `MPC-TDF-002` 해소 기록과 protocol golden | 모든 command/query result가 versioned golden과 일치 | result shape 미정, adapter별 임의 해석 |

### 3.6 Repo evidence freshness와 경로 보안

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-EVD-001` | BLOCKING | 자동 | clean, dirty, detached HEAD, unborn Git repository를 모두 허용하고 identity를 기록한다. | Git fixture matrix | 각 상태에서 snapshot 생성 | 특정 허용 상태를 오류 처리 |
| `MPC-EVD-002` | BLOCKING | 자동 | dirty snapshot은 정확한 `status_hash`와 cited file hash에 결박된다. | dirty file variation | 한 byte 변화로 freshness false | dirty 여부만 저장하고 내용 변화 미감지 |
| `MPC-EVD-003` | BLOCKING | 자동 | non-Git project는 canonical root와 cited file hash로 identity를 만든다. | non-Git fixture | Git 호출 실패 없이 동작 | Git 필수 또는 모든 파일 변화로 stale |
| `MPC-EVD-004` | BLOCKING | 자동 | root 밖 path, root 밖 symlink, `.megara/`, `.git/`, ignored file이 거부된다. | path attack fixture | 모든 공격 typed error, event +0 | path traversal 또는 symlink escape 허용 |
| `MPC-EVD-005` | BLOCKING | 자동 | secret·credential 계열 파일이 evidence로 거부되고 추적된 `.env.example` 예외가 적용된다. | `MPC-TDF-005`에 따른 fixture | deny/allow 목록 전부 계약대로 | 패턴 미정 또는 실제 secret path 허용 |
| `MPC-EVD-006` | BLOCKING | 자동 | DB에는 source 원문 전체가 아니라 path, range, size, digest, tracked 상태, captured_at만 저장된다. | DB byte/string scan | fixture source 전체 문장이 DB에 없음 | 파일 원문 전체 복사 |
| `MPC-EVD-007` | BLOCKING | 자동 | status/current/show가 현재 HEAD·status·cited digest를 읽어 `ObservedHealth`를 계산하되 event를 만들지 않는다. | query 전후 DB count/hash | warning만 반환, revision 불변 | query mutation |
| `MPC-EVD-008` | BLOCKING | 자동 | full audit, spec generate/approve, plan generate/approve, approved bundle export는 stale evidence에서 차단된다. | repo mutation 후 operation matrix | 모두 `EVIDENCE_STALE`, event +0 | 하나라도 stale 상태에서 성공 |
| `MPC-EVD-009` | BLOCKING | 자동 | 동일 evidence refresh는 event와 revision을 만들지 않는다. | 동일 citation 재제출 | event/revision 불변 | no-op event 생성 |
| `MPC-EVD-010` | BLOCKING | 자동 | 변경 evidence refresh는 한 aggregate event로 invalidation하고 Interview로 복귀한다. | HEAD/status/file mutation | pending/work item 취소, domain +1, artifact revoke | 부분 invalidation 또는 기존 phase 유지 |
| `MPC-EVD-011` | BLOCKING | 자동 | 영향 범위 불명확 시 모든 repo-derived Fact와 downstream을 stale 처리한다. | unknown dependency fixture | 보수적 invalidation | 일부 최신성을 근거 없이 유지 |
| `MPC-EVD-012` | BLOCKING | 자동 | citation line range가 1-based inclusive이며 잘못된 range와 missing file이 거부된다. | citation boundary fixtures | 정확한 digest/range 저장 | 0-based, 역전 range, 없는 파일 수용 |
| `MPC-EVD-013` | INFORMATIONAL | 수동 | 외부 웹 자료는 자동 freshness source가 아니며 UserAnswer 또는 Assumption으로만 canonical화된다는 한계를 문서화한다. | docs review | 한계와 사용법 명시 | 사용자가 외부 자료 freshness를 자동 보장한다고 오해할 표현 |

### 3.7 QuestionProposal, 초심자 UX, QuestionProjection provenance

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-QST-001` | BLOCKING | 자동 | next question 가능 work item마다 `megara.question-authoring/v1`과 일곱 규칙이 동일 순서·원문으로 포함된다. | work item golden, adapter contract test | 두 adapter 입력 동일 | adapter별 누락·변경 |
| `MPC-QST-002` | BLOCKING | 자동 | 질문은 한 번에 하나만 pending이며 one-decision rubric을 모델에 전달한다. | multi-question proposal | schema 거부 또는 단일 질문 | 여러 결정을 한 pending question에 구조적으로 병합 |
| `MPC-QST-003` | BLOCKING | 자동 | 공통 필드 `context`, `question`, `why_it_matters`, `technical_terms`, `source_refs`, `answer`가 exact schema를 따른다. | valid/invalid golden | 필수 필드·trim 검증 | missing/blank 허용 |
| `MPC-QST-004` | BLOCKING | 자동 | choice mode는 최소 2개 choice와 각 `label`, `direction`, non-empty `benefits`, `tradeoffs`를 요구한다. | cardinality/blank fixtures | 잘못된 choice 전체 거부 | label-only 또는 빈 배열 허용 |
| `MPC-QST-005` | BLOCKING | 자동 | recommendation은 null 또는 완전한 object이며 같은 choice와 유효한 source를 참조한다. | partial/unknown choice fixtures | 부정확한 참조 거부 | source 없는 추천 허용 |
| `MPC-QST-006` | BLOCKING | 자동 | freeform mode는 choices·recommendation을 허용하지 않고 `freeform_hint`가 필수다. | tagged-union fixtures | unknown field 전체 거부 | variant 혼합 |
| `MPC-QST-007` | BLOCKING | 자동 | technical term은 중복되지 않는 non-empty term과 non-empty plain explanation을 가진다. | NFC/duplicate/blank fixtures | 구조 위반 거부 | 설명 없는 전문용어 수용 |
| `MPC-QST-008` | BLOCKING | 자동 | invalid QuestionProposal은 event, revision, pending question을 바꾸지 않고 required work item을 유지한다. | before-after state | 모든 authoritative 값 동일 | 일부 question/entity 적용 |
| `MPC-QST-009` | BLOCKING | 자동 | 수정 proposal은 같은 work item/base/input hash와 새 command ID로 재제출할 수 있다. | invalid→corrected sequence | corrected proposal 1회 적용 | work item 유실 또는 자동 retry |
| `MPC-QST-010` | MANUAL | 수동 | gold fixture가 초심자 rubric 일곱 항목을 모두 충족하고 anti-example은 각 실패 이유가 rubric 항목에 연결된다. | 서명된 fixture review | 7/7 `예`, anti-example 실패 mapping 완전 | 하나라도 `아니오`, 근거 없는 통과 |
| `MPC-QST-011` | BLOCKING | 자동 | QuestionProjection block 순서가 technical term→context→question→why→choice→recommendation→freeform이다. | pure projection golden | 순서와 optional block 수 일치 | 순서 변경·누락·중복 |
| `MPC-QST-012` | BLOCKING | 자동 | 모든 사용자 표시 필드가 정확히 한 번 투영되고 choice ID·recommendation target이 보존된다. | field occurrence assertion | 각 field count=1 | 누락, 중복, ID 손실 |
| `MPC-QST-013` | BLOCKING | 자동 | question source와 recommendation source가 지정 metadata 위치에 분리 보존된다. | provenance golden | 서로 섞이지 않고 1회 저장 | metadata 삭제·본문 중복 |
| `MPC-QST-014` | BLOCKING | 자동 | Codex와 Pi가 같은 normalized semantic block·metadata를 재작성 없이 전달한다. | adapter equivalence test | semantic sequence 동일 | 요약, 병합, 문구 재작성 |
| `MPC-QST-015` | MANUAL | 수동 | 불가피한 전문용어 설명이 단순 약어 확장이 아니라 역할과 중요성을 초심자가 이해할 수준으로 설명한다. | signed UX review | 모든 term에 문맥 역할·영향 설명 | `WAL=write-ahead log` 수준의 설명 |

### 3.8 Audit, readiness, spec, plan, approval

#### Audit와 readiness

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-AUD-001` | BLOCKING | 자동 | DeltaAudit는 `continue` 또는 `request_full_audit`만 제안하며 직접 ready 전환하지 않는다. | schema fixtures | 금지 조합 거부 | delta로 Specification 진입 |
| `MPC-AUD-002` | BLOCKING | 자동 | DeltaAudit `continue`에는 질문이 필수이고 `request_full_audit`에는 질문이 null이다. | combination matrix | 유효 조합만 적용 | next action 0개·2개 |
| `MPC-AUD-003` | BLOCKING | 자동 | FullAudit는 `counterexample_review`가 필수이고 delta에서는 null이다. | mode fixtures | mode별 exact schema | 누락 또는 delta에 허용 |
| `MPC-AUD-004` | BLOCKING | 자동 | blocking finding에는 대응 blocking BlockerOp가 반드시 존재한다. | finding/blocker mismatch | 불일치 전체 거부 | finding만 기록하고 readiness 통과 |
| `MPC-AUD-005` | BLOCKING | 자동 | FullAudit가 entity/edge/blocker를 바꾸면 Interview에 남고 새 audit가 필요하다. | full audit mutation fixture | domain 증가, new work item | 변경과 동시에 Specification 진입 |
| `MPC-AUD-006` | BLOCKING | 자동 | §13.4 readiness gate의 모든 조건을 독립적으로 차단 테스트한다. | 조건별 one-missing fixture | 각 누락이 Specification 전환 차단 | 누락 조건 하나라도 통과 |
| `MPC-AUD-007` | BLOCKING | 자동 | 숫자 score, threshold, floor, streak가 schema·state·gate에 존재하지 않는다. | source/schema scan | 관련 production field 0 | 수치 gate 잔존 |
| `MPC-AUD-008` | BLOCKING | 자동 | current audit input hash와 base revision이 맞지 않는 proposal은 state 변화 없이 거부된다. | stale audit fixture | `PROPOSAL_BASE_MISMATCH`, event +0 | stale proposal 적용 |

#### Spec

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-SPC-001` | BLOCKING | 자동 | current full audit 없이 spec candidate를 생성할 수 없다. | no/full-stale audit fixtures | generate 거부 | spec 생성 성공 |
| `MPC-SPC-002` | BLOCKING | 자동 | SpecProposal은 current entity revision만 조립하고 새 의미를 만들거나 수정하지 않는다. | new body/unknown ref fixture | 전체 거부 | proposal에서 requirement 변경 |
| `MPC-SPC-003` | BLOCKING | 자동 | Requirement와 AcceptanceCriterion graph 연결, 중복 ref, stale ref를 재검증한다. | malformed spec fixtures | candidate 미생성 | 구조 오류 candidate 생성 |
| `MPC-SPC-004` | BLOCKING | 자동 | semantic hash는 NFC, LF, trailing whitespace normalization을 따르고 내부 의미 변화에는 달라진다. | canonicalization golden | 허용 formatting 변화 동일, 의미 변화 상이 | 반대 결과 |
| `MPC-SPC-005` | BLOCKING | 자동 | candidate가 base domain revision, audit input hash, entity revision refs를 보존한다. | DB/event/candidate inspection | 모든 binding 존재 | 하나라도 누락 |
| `MPC-SPC-006` | BLOCKING | 자동 | spec approve가 candidate ID, semantic hash, base domain revision을 정확히 비교한다. | 각 필드 1개씩 변조 | 모두 approval event 0 | 일부만 확인 |
| `MPC-SPC-007` | BLOCKING | 자동 | stale candidate, stale evidence, blocker 존재 시 spec approval이 차단된다. | condition matrix | 정확한 typed error | 승인 생성 |
| `MPC-SPC-008` | BLOCKING | 자동 | 모델 문구나 model-facing RPC가 approval event를 만들 수 없다. | malicious proposal/RPC | `USER_ENTRYPOINT_REQUIRED` 또는 schema 거부 | 모델 경로 승인 성공 |
| `MPC-SPC-009` | BLOCKING | 자동 | spec revise가 Interview 복귀, domain 증가, full audit·spec·plan invalidation을 한 aggregate event에 기록한다. | revise fixture | effects와 state 일치 | plan approval 잔존 |
| `MPC-SPC-010` | BLOCKING | 자동 | approval actor·time은 metadata에만 있고 normalized state에는 포함되지 않는다. | two-adapter approval fixture | normalized state 동일 | actor 차이로 state hash 변경 |

#### Plan

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-PLN-001` | BLOCKING | 자동 | current approved spec 없이 PlanProposal을 생성할 수 없다. | no/stale spec fixture | `INVALID_PHASE` 또는 candidate error | plan 생성 |
| `MPC-PLN-002` | BLOCKING | 자동 | PlanProposal의 baseline, step, verification, risk 필드가 exact schema를 따른다. | golden/unknown field fixtures | 누락·unknown 전체 거부 | 기본값 자동 삽입 |
| `MPC-PLN-003` | BLOCKING | 자동 | 모든 PlanStep이 하나 이상의 current Requirement를 참조한다. | orphan step fixture | candidate blocked/거부 | orphan step 승인 가능 |
| `MPC-PLN-004` | BLOCKING | 자동 | 모든 current Requirement가 최소 한 PlanStep에 연결된다. | uncovered requirement fixture | structural blocker | 누락을 통과 |
| `MPC-PLN-005` | BLOCKING | 자동 | 모든 current AcceptanceCriterion이 Verification에 연결되고 Verification은 PlanStep에 연결된다. | orphan AC/verification fixtures | 구조 오류 차단 | 누락 통과 |
| `MPC-PLN-006` | BLOCKING | 자동 | dependency missing reference와 cycle을 차단한다. | DAG/cycle fixtures | cycle candidate 승인 불가 | cycle 허용 |
| `MPC-PLN-007` | BLOCKING | 자동 | plan input hash가 approved spec ID/hash, evidence hash, plan revision, schema version에 결박된다. | 한 입력씩 변경 | candidate stale/생성 거부 | 이전 hash plan 사용 |
| `MPC-PLN-008` | BLOCKING | 자동 | plan approve가 candidate ID, semantic hash, base plan revision을 정확히 비교한다. | 각 field mismatch | approval event 0 | 일부 비교 누락 |
| `MPC-PLN-009` | BLOCKING | 자동 | plan approve 시 current spec, evidence, structural blockers를 다시 검사한다. | approval 직전 mutation fixture | stale 승인 차단 | generate 때만 검사 |
| `MPC-PLN-010` | BLOCKING | 자동 | plan revise는 plan revision과 plan candidate/approval만 stale 처리하고 spec approval은 유지한다. | revise fixture | spec approval current | spec까지 revoke하거나 plan 유지 |
| `MPC-PLN-011` | BLOCKING | 자동 | planner·architect·critic이 runtime state나 subagent gate로 존재하지 않는다. | source/runtime negative proof | GeneratePlan 1회+Rust validator | 역할 fan-out·subagent state |
| `MPC-PLN-012` | MANUAL | 수동 | 구조적으로 유효한 adversarial plan을 의미적 품질 보증으로 표시하지 않는다. | adversarial fixture와 UI/docs review | 구조 통과와 의미 검토 필요를 구분 | “검증된 고품질 계획” 등 과장 |

### 3.9 Markdown projection, export, purge

#### Projection

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-PRJ-001` | BLOCKING | 자동 | spec.md·plan.md에 generated header, session, candidate, hash, base revision이 포함된다. | renderer golden | 모든 필드 존재 | header 누락·오류 |
| `MPC-PRJ-002` | BLOCKING | 자동 | direct edit가 canonical state를 변경하지 않고 `projection_status=conflict`를 만든다. | file edit fixture | DB/event/hash 불변 | edit 자동 import |
| `MPC-PRJ-003` | BLOCKING | 자동 | write는 temp→write/fsync→digest check→atomic rename 순서이며 crash residue가 복구 가능하다. | crash points·temp file fixture | 기존 정상 파일 유지 또는 재생성 | 반쪽 파일로 교체 |
| `MPC-PRJ-004` | BLOCKING | 자동 | projection 삭제 후 DB/event만으로 동일 Markdown을 재생성한다. | delete→show/doctor repair | renderer output/hash 일치 | artifact 소실 |
| `MPC-PRJ-005` | BLOCKING | 자동 | projection conflict·I/O failure가 이미 commit된 DB mutation을 실패·rollback으로 표현하지 않는다. | read-only/permission fixture | ok result+warning, candidate 존재 | error로 mutation 부재처럼 응답 |
| `MPC-PRJ-006` | BLOCKING | 자동 | `--force`는 projection policy일 뿐 request hash·core invariant에 포함되지 않는다. | same command ID protect→force | event 1개, projection만 재시도 | command reuse error 또는 새 event |
| `MPC-PRJ-007` | BLOCKING | 자동 | adapter나 renderer가 Markdown을 canonical input으로 parse하지 않는다. | malformed manual edit fixture | state 불변 | parsed entity 변경 |

#### Export

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-EXP-001` | BLOCKING | 자동 | 기본 bundle은 manifest, 존재하는 spec, plan만 포함한다. | archive/file tree inspection | raw transcript/event 없음 | 민감 원문 포함 |
| `MPC-EXP-002` | BLOCKING | 자동 | `--include-transcript` 없이는 answer, model proposal, semantic event payload가 제외된다. | secret marker scan | marker 0 | raw content 노출 |
| `MPC-EXP-003` | BLOCKING | 자동 | 기존 output은 `--force` 없이 덮어쓰지 않는다. | sentinel file fixture | sentinel 유지, conflict error | overwrite |
| `MPC-EXP-004` | BLOCKING | 자동 | approved bundle은 stale evidence에서 차단된다. | repo mutation 후 export | `EVIDENCE_STALE`, output 없음 | stale bundle 생성 |
| `MPC-EXP-005` | BLOCKING | 자동 | state-json·events-jsonl recovery export는 stale 상태에서도 허용된다. | stale fixture | recovery output 생성 | bundle 규칙으로 모두 차단 |
| `MPC-EXP-006` | MANUAL | 수동 | export manifest가 생성 출처와 민감정보 포함 여부를 사용자가 식별할 수 있게 설명한다. | exported manifest review | session/artifact/include-transcript 상태 명확 | 포함 범위를 알 수 없음 |

#### Purge

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-PUR-001` | BLOCKING | 자동 | purge confirmation과 expected revision이 정확하지 않으면 삭제가 없다. | mismatch fixtures | `PURGE_CONFIRMATION_MISMATCH`/revision error | 일부 row/file 삭제 |
| `MPC-PUR-002` | BLOCKING | 자동 | purge transaction이 session event, session row, command results를 삭제하고 receipt와 retired IDs를 남긴다. | `ER-PURGE` DB inspection | 계획된 row 상태 일치 | content row 잔존 또는 receipt 누락 |
| `MPC-PUR-003` | BLOCKING | 자동 | tombstone clean 상태에는 최소 필드만 남고 제목·transcript·entity·path·artifact hash가 없다. | DB schema/byte scan | 금지 marker 0 | 민감 metadata 잔존 |
| `MPC-PUR-004` | BLOCKING | 자동 | linked migration backup이 기본 삭제된다. | imported session purge fixture | backup directory 없음 | backup 잔존 |
| `MPC-PUR-005` | BLOCKING | 자동 | 같은 purge command ID·같은 request는 receipt를 replay한다. | purge retry | `replayed=true`, 새 row 변화 없음 | `SESSION_PURGED`만 반환하거나 새 receipt 생성 |
| `MPC-PUR-006` | BLOCKING | 자동 | purge command ID의 다른 request는 `COMMAND_ID_REUSE`다. | request 변조 | typed error | 재사용 허용 |
| `MPC-PUR-007` | BLOCKING | 자동 | 삭제된 과거 command ID는 `COMMAND_ID_RETIRED`다. | retired command replay | state 변화 0 | 새 command로 재사용 |
| `MPC-PUR-008` | BLOCKING | 자동 | artifact/backup/checkpoint/VACUUM 실패 시 logical purge 유지, cleanup pending을 보고한다. | post-commit failure injection | session 접근 불가, pending receipt 존재 | session 부활 또는 성공 clean 오보고 |
| `MPC-PUR-009` | BLOCKING | 자동 | doctor `--repair`가 pending residue를 재시도하고 완료 뒤 최소 tombstone으로 축소한다. | pending→repair fixture | residue 제거, pending ID null | event 복원·민감정보 잔존 |
| `MPC-PUR-010` | BLOCKING | 자동 | `secure_delete`, DELETE, WAL truncate, VACUUM 후 DB/WAL/artifact/backup에서 secret marker가 검출되지 않는다. | byte scan 원문 | marker 0 | 한 위치라도 검출 |
| `MPC-PUR-011` | BLOCKING | 자동 | partial redaction command나 hidden 선택 삭제가 존재하지 않는다. | CLI/schema/source negative proof | session purge만 제공 | 부분 event 삭제 기능 존재 |
| `MPC-PUR-012` | INFORMATIONAL | 수동 | SSD wear-leveling과 외부 backup을 포함한 forensic erase가 비보증 범위임을 명시한다. | docs/security review | 한계 명시 | 완전한 장치-level 삭제를 보증한다고 주장 |

### 3.10 Codex MCP와 Pi adapter

#### 공통 adapter

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-ADP-001` | BLOCKING | 자동 | Codex와 Pi는 같은 PlanningService command/query contract를 사용한다. | adapter equivalence traces | normalized state·semantic events 동일 | adapter별 다른 state transition |
| `MPC-ADP-002` | BLOCKING | 자동 | adapter가 모델 output을 수정·보완·기본값 삽입하지 않는다. | invalid proposal through adapters | core가 동일 schema error 반환 | adapter가 유효한 proposal로 변환 |
| `MPC-ADP-003` | BLOCKING | 자동 | adapter가 DB, event, artifact를 직접 쓰지 않는다. | source scan·filesystem tracing | 직접 write 0 | adapter write 검출 |
| `MPC-ADP-004` | BLOCKING | 자동 | adapter가 daemon, background queue, automatic retry loop를 생성하지 않는다. | process tree·source review | 요청 종료 후 child/background 0 | process 잔존·hidden retry |
| `MPC-ADP-005` | BLOCKING | 자동 | planning adapter 설치가 lifecycle hook을 등록하지 않는다. | clean install tree/config | hooks projection 0 | hooks.json 또는 hook entry 생성 |
| `MPC-ADP-006` | BLOCKING | 자동 | current host model과 thinking level을 그대로 사용한다. | host configuration before-after | 값 불변 | adapter 변경 |
| `MPC-ADP-007` | BLOCKING | 자동 | model-facing path에서 approve·purge가 거부된다. | Pi RPC/MCP misuse fixtures | `USER_ENTRYPOINT_REQUIRED` 또는 host prompt | silent 승인·purge |
| `MPC-ADP-008` | BLOCKING | 혼합 | 승인·purge는 CLI direct, Codex prompt tool, Pi slash confirmation 경로에서만 성공한다. | automated entrypoint tests + `ER-HOST` | 허용 경로만 event 생성 | 모델 tool이나 임의 RPC 성공 |
| `MPC-ADP-009` | MANUAL | 수동 | 실제 Codex host가 approve·purge 전에 confirmation을 표시한다. | 화면/host log, reviewer signoff | candidate/hash/revision 또는 purge 대상 확인 후 사용자 동작 | prompt 없음·자동 승인 |
| `MPC-ADP-010` | MANUAL | 수동 | 실제 Pi UI가 candidate ID/hash/base revision 또는 purge 대상 session을 표시하고 확인 후 CLI를 실행한다. | 화면/command log, signoff | 표시값과 실행 CLI 값 일치 | 표시 누락·다른 대상 실행 |
| `MPC-ADP-011` | BLOCKING | 자동 | adapter equivalence는 raw event ID가 아니라 normalized state와 semantic event sequence를 비교한다. | equivalence report | 두 기준 모두 동일 | raw UUID 동일성만 확인 |
| `MPC-ADP-012` | BLOCKING | 자동 | Codex/Pi가 같은 QuestionProjection block과 provenance metadata를 전달한다. | normalized block comparison | block kind·순서·metadata 동일 | adapter별 누락·재작성 |
| `MPC-ADP-013` | BLOCKING | 자동 | generated UUID·actor·timestamp 차이는 허용하되 semantic 관계는 보존된다. | alias fixture | normalized hash 동일 | 관계 손실 또는 metadata 포함 |
| `MPC-ADP-014` | BLOCKING | 혼합 | 지원 대상 host version별 adapter E2E가 실행된다. | `MPC-TDF-004` 해소 후 host matrix | 모든 지원 version 통과 | 지원 version 미검증 |
| `MPC-ADP-015` | BLOCKING | 혼합 | install·update·remove가 managed projection만 변경하며 user-modified file을 보호한다. | `MPC-TDF-003`, install/update/remove FS diff | 계약된 entrypoint별 파일 diff 일치 | 제거 entrypoint 미정·사용자 파일 삭제 |

#### Codex MCP

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-MCP-001` | BLOCKING | 자동 | MCP initialize, tools/list, tools/call이 conformance test를 통과한다. | `VB-ADAPTER`, protocol traces | 모든 호출 parse·typed response | framing 오류·panic |
| `MPC-MCP-002` | BLOCKING | 자동 | `.codex/config.toml`에 absolute binary path, project path, cwd, enabled, timeout 값이 계획대로 등록된다. | generated config golden | 모든 key 일치 | relative path·누락 |
| `MPC-MCP-003` | BLOCKING | 자동 | `toml_edit` merge가 다른 key, server, comment, ordering을 보존한다. | sentinel config diff | managed table 외 byte/semantic 보존 | 전체 TOML 재직렬화·comment 소실 |
| `MPC-MCP-004` | BLOCKING | 자동 | unmanaged same-name table은 `--force` 없이 conflict다. | config fixture | 기존 config 불변 | silent overwrite |
| `MPC-MCP-005` | BLOCKING | 자동 | `--force`는 해당 table만 backup 후 교체한다. | backup/file diff | backup byte-equivalent, 다른 key 보존 | config 전체 교체·backup 없음 |
| `MPC-MCP-006` | BLOCKING | 자동 | approve·purge tool에 `approval_mode=prompt`와 적절한 annotation이 설정된다. | config/tool metadata | 세 tool 모두 prompt | 하나라도 자동 실행 가능 |
| `MPC-MCP-007` | BLOCKING | 수동 | initialize instructions 첫 문장이 planning-only·typed proposal·승인 추정 금지를 전달한다. | MCP initialize output review | 세 의미 모두 포함 | 실행 workflow 유도·승인 추정 허용 |
| `MPC-MCP-008` | BLOCKING | 자동 | generic arbitrary mutation tool을 제공하지 않는다. | tools/list snapshot | 명시된 tool만 존재 | actor/operation을 자유 입력하는 mutation tool |

#### Pi

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-PI-001` | BLOCKING | 자동 | 각 Pi tool/slash command가 one-shot RPC 또는 exact CLI child를 실행하고 종료한다. | process lifecycle trace | 요청 종료 후 child 0 | persistent process |
| `MPC-PI-002` | BLOCKING | 자동 | model-facing `registerTool`에 approve·purge가 등록되지 않는다. | tool registry snapshot | 금지 tool 0 | approve/purge tool 존재 |
| `MPC-PI-003` | BLOCKING | 자동 | slash command가 status에서 exact candidate binding을 읽고 확인 후 동일 값을 CLI에 전달한다. | mocked confirmation trace | 표시값=실행값 | stale/다른 candidate 실행 |
| `MPC-PI-004` | BLOCKING | 자동 | role process, fallback model, retry loop, subagent tool 코드가 제거됐다. | source/runtime negative proof | 관련 경로·등록 0 | 잔존 |
| `MPC-PI-005` | BLOCKING | 자동 | RPC error, timeout, child failure가 typed 실패로 표시되고 state를 추정 변경하지 않는다. | failure fixtures | event +0, 오류 전달 | silent retry·성공 오보고 |
| `MPC-PI-006` | BLOCKING | 혼합 | Pi child process timeout·termination 계약이 구현되고 검증된다. | `MPC-TDF-006` 해소 후 timeout fixture | 제한 내 종료·강제 종료 후 child 0 | 무기한 hang |
| `MPC-PI-007` | BLOCKING | 자동 | Pi가 current model과 thinking level을 변경하는 API를 호출하지 않는다. | source scan·host log | 호출 0 | 변경 호출 존재 |
| `MPC-PI-008` | BLOCKING | 자동 | Pi extension이 `.megara/planning` 또는 Codex config를 직접 조작하지 않는다. | source/file tracing | 직접 접근 0 | path 직접 write |

### 3.11 Migration, resume, rollback, legacy 자동 승인 금지

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-MIG-001` | BLOCKING | 자동 | `--dry-run`은 inventory와 계획만 출력하고 filesystem·DB write가 0이다. | full tree/DB hash before-after | byte-identical | directory, manifest, DB 생성 |
| `MPC-MIG-002` | BLOCKING | 자동 | apply 전에 legacy state, artifact, managed projection inventory가 완전하다. | Slice 0 fixture와 manifest 비교 | 누락 path 0 | 미분류 파일 존재 |
| `MPC-MIG-003` | BLOCKING | 자동 | backup은 removal 전 byte-for-byte 완료되고 manifest에 source hash가 기록된다. | source/backup hash | 전부 일치 | 하나라도 누락·불일치 |
| `MPC-MIG-004` | BLOCKING | 자동 | backup 실패 시 removal, DB import, projection 변경이 0이다. | injected IO failure | 원본·DB 불변 | 일부 제거·session 생성 |
| `MPC-MIG-005` | BLOCKING | 자동 | journal 상태가 prepared→planning_imported→projection_removed→applied 또는 rolled_back으로만 이동한다. | transition tests | illegal jump 거부 | 단계 생략·역행 |
| `MPC-MIG-006` | BLOCKING | 자동 | manifest update는 temp+fsync+atomic rename이며 partial manifest를 만들지 않는다. | crash injection | 이전 또는 새 완전 manifest | truncated JSON |
| `MPC-MIG-007` | BLOCKING | 자동 | 각 journal 단계 중단 후 `--resume`이 마지막 완료 단계부터 중복 없이 진행한다. | crash point별 fixture | session·backup·removal 중복 0 | duplicate session·파일 손실 |
| `MPC-MIG-008` | BLOCKING | 자동 | DB commit 후 manifest write 전 crash를 derived command ID replay로 복구한다. | 정확한 crash fixture | session 1개, revision 1 | session 2개 또는 import 유실 |
| `MPC-MIG-009` | BLOCKING | 자동 | imported session의 첫 event는 `LegacyContextImported` 하나이며 revision=1이다. | event query | `PlanningSessionStarted` 추가 없음 | event 2개 또는 다른 phase |
| `MPC-MIG-010` | BLOCKING | 자동 | legacy raw context는 opaque이고 entity/spec/plan/approval로 자동 변환되지 않는다. | imported DB inspection | Interview, DeltaAudit work item, approval 없음 | candidate·approval 자동 생성 |
| `MPC-MIG-011` | BLOCKING | 자동 | user-modified managed file은 `--force` 없이 보존되고 warning으로 보고된다. | modified file fixture | original hash 유지 | 삭제·overwrite |
| `MPC-MIG-012` | BLOCKING | 자동 | rollback은 intermediate 단계에서 완료된 동작을 역순으로 되돌리고 byte-equivalent 원본을 복원한다. | 각 단계 rollback fixture | source tree hash 원본과 동일 | 파일·state 누락 |
| `MPC-MIG-013` | BLOCKING | 자동 | migration-created session이 변경됐으면 `ROLLBACK_CONFLICT`, `--force`는 export 후 purge한다. | changed session fixture | export 존재 후 purge | 변경 내용 무통보 삭제 |
| `MPC-MIG-014` | BLOCKING | 자동 | 기존 `.agents/state`→`.megara/state` migration 책임이 유지된다. | existing migration regression fixture | 이전 동작 통과 | planning migration이 기존 경로를 누락 |
| `MPC-MIG-015` | BLOCKING | 자동 | clean install에는 legacy workflow Skill·hook·Team·Ultragoal projection이 없다. | clean install tree | 금지 파일·등록 0 | 하나라도 설치 |
| `MPC-MIG-016` | BLOCKING | 혼합 | rollback 운영 절차가 이전 Megara release 재설치 필요성을 포함해 재현 가능하다. | E2E rollback + operator review | 문서 절차로 복원 성공 | 숨은 수동 단계·복원 실패 |

### 3.12 기존 install/update/doctor 회귀와 파일 보호

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-REG-001` | BLOCKING | 자동 | Slice 0에서 기존 install/update/doctor 동작과 managed file snapshot을 기록한다. | `VB-BASELINE` | snapshot 재생성 가능 | baseline 없음 |
| `MPC-REG-002` | BLOCKING | 자동 | planning 변경 후 비대상 install/update/doctor 기능에 신규 실패가 없다. | baseline vs release 비교 | 기존 성공 유지 | 신규 회귀 |
| `MPC-REG-003` | BLOCKING | 자동 | `harness/.gitignore`와 runtime `.megara/.gitignore`에 state, artifacts, cache, planning, migration-backups가 있다. | file golden·install output | 두 위치 동일 계약 | 누락·불일치 |
| `MPC-REG-004` | BLOCKING | 자동 | DB, WAL, SHM, artifact, migration backup이 실제 Git ignore 대상이다. | installed project ignore assertion | 모두 ignored | 하나라도 tracked candidate |
| `MPC-REG-005` | BLOCKING | 자동 | `--force` 없이 기존 사용자 파일을 덮어쓰거나 삭제하지 않는다. | sentinel files, install/update/migrate | byte-identical 보존 | overwrite·삭제 |
| `MPC-REG-006` | BLOCKING | 자동 | doctor 기본은 read-only이고 repair는 event를 수정하지 않는다. | file/DB hash before-after | 계약 일치 | 기본 write·event rewrite |
| `MPC-REG-007` | BLOCKING | 자동 | 계획 §21의 module 책임 제한을 지킨다. | source dependency assertions | templates/targets/ratatui 경계 위반 0 | planning logic이 금지 모듈에 존재 |
| `MPC-REG-008` | BLOCKING | 자동 | ratatui는 install/update/doctor command-scoped adapter 밖에서 planning을 import하지 않는다. | source/dependency scan | planning TUI import 0 | planning session UI 추가 |
| `MPC-REG-009` | BLOCKING | 자동 | 신규 unit/integration module이 기존 두 test target의 `main.rs`에 명시적으로 등록된다. | test module inventory | 계획의 모든 파일 실행 | 파일은 있으나 test target 미등록 |
| `MPC-REG-010` | BLOCKING | 자동 | docs 검증과 `git diff --check`가 release commit에서 통과한다. | `VB-RELEASE` | exit 0 | docs link/frontmatter/whitespace 실패 |

### 3.13 Protocol resource limit, 악성 입력, 성능·장시간 lock

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-RES-001` | BLOCKING | 자동 | 최대 허용 payload와 10,000 operation proposal이 panic·partial apply 없이 처리된다. | boundary stress fixture | 성공 또는 계약된 오류, event 원자성 | OOM/panic/부분 state |
| `MPC-RES-002` | BLOCKING | 자동 | 초과 payload, text, path, ID, operation은 parse/apply 전에 거부된다. | 초과 corpus | event/revision +0 | 일부 entity 적용 |
| `MPC-RES-003` | BLOCKING | 자동 | 악성 unknown field, duplicate temp_ref, dangling ref, invalid Unicode/JSON이 state를 바꾸지 않는다. | malicious corpus | typed error, process 생존 | panic·state corruption |
| `MPC-RES-004` | BLOCKING | 자동 | deeply nested JSON에 대한 parser recursion/resource 제한이 확정되고 테스트된다. | `MPC-TDF-008` 해소 후 nesting corpus | 제한 초과 안전 거부 | stack overflow·hang |
| `MPC-RES-005` | BLOCKING | 자동 | DB lock 5초 timeout이 정확히 적용되고 종료 후 lock/process residue가 없다. | lock test | `DB_BUSY`, child/lock 해제 | 무기한 대기·잔여 lock |
| `MPC-RES-006` | BLOCKING | 자동 | Codex MCP startup 10초, tool 120초 설정이 projection과 host test에 반영된다. | config golden·timeout fixture | 값 일치, timeout 보고 | 다른 값·hang |
| `MPC-RES-007` | BLOCKING | 혼합 | Pi child timeout과 termination 정책이 정의·구현됐다. | `MPC-TDF-006`, process trace | timeout 후 child 0, typed failure | 무기한 실행 |
| `MPC-RES-008` | BLOCKING | 혼합 | normal/replay/migration/purge 작업의 성능·장시간 lock 예산이 정의되고 통과한다. | `MPC-TDF-007` 해소 후 benchmark report | 모든 예산 충족 | 예산 미정·초과 |
| `MPC-RES-009` | BLOCKING | 자동 | migration·purge 장시간 작업 중 중단해도 journal/pending cleanup으로 resume 가능하다. | large fixture interruption | 데이터 손실 없이 resume | 중간 상태 복구 불가 |
| `MPC-RES-010` | INFORMATIONAL | 수동 | v1 resource limit와 비보증 범위가 사용자 문서에 명시된다. | docs review | 4MiB·field/op 제한, secure erase 한계 명시 | 제한을 알 수 없음 |

### 3.14 Unit, integration, fixture, golden, E2E, release evidence

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-BAS-001` | BLOCKING | 자동 | base commit SHA, `Cargo.lock` hash, toolchain, command별 exit code가 기록된다. | `VB-BASELINE` manifest | 필드 완전 | 하나라도 누락 |
| `MPC-BAS-002` | BLOCKING | 혼합 | baseline failure가 base checkout에서 독립 재현된다. | base/release raw logs | 계획 §24 조건 모두 만족 | release 로그만 존재 |
| `MPC-BAS-003` | BLOCKING | 자동 | legacy managed path fixture가 재실행 시 동일하다. | fixture regeneration diff | diff 0 | 비결정적 또는 누락 |
| `MPC-BAS-004` | BLOCKING | 자동 | baseline 이후 신규 failure가 0이다. | `VB-RELEASE` 비교 | 신규 실패 0 | 하나 이상 |
| `MPC-BAS-005` | BLOCKING | 자동 | planning test는 baseline allow-list로 제외되지 않는다. | allow-list review·test report | planning 실패 0, waiver 0 | 제외·skip |
| `MPC-BAS-006` | INFORMATIONAL | 수동 | 남은 baseline failure와 사용자 영향이 Release Decision Record에 기록된다. | decision record | 원인·영향·재현 링크 존재 | 숨김 |
| `MPC-TST-001` | BLOCKING | 자동 | §23 test matrix 각 행이 하나 이상의 실행된 test ID와 연결된다. | `ER-TRACE` | 미연결 matrix row 0 | trace 누락 |
| `MPC-TST-002` | BLOCKING | 자동 | 계획 §21.3의 모든 test module이 실제 test target에 등록·실행된다. | test list/report | module별 실행 test >0 | 파일만 존재 |
| `MPC-TST-003` | BLOCKING | 자동 | 필수 테스트 중 ignored·skipped·filtered-out 항목이 0이다. | full test report | 0 | 하나 이상 |
| `MPC-TST-004` | BLOCKING | 자동 | fixture와 golden이 테스트 실행 중 자기 자신을 기대값으로 재생성하지 않는다. | test source review, immutable golden hash | 별도 expected source 사용 | 실행 중 expected overwrite |
| `MPC-TST-005` | BLOCKING | 자동 | 각 오류·경계 테스트가 state/event/revision 불변을 assertion한다. | assertion trace | 오류 후 authoritative delta 0 | error code만 확인 |
| `MPC-TST-006` | BLOCKING | 자동 | crash 테스트가 transaction 전·중·후와 DB commit 후 projection/manifest 실패를 각각 다룬다. | crash matrix report | 모든 지점 실행 | 대표 지점 하나만 테스트 |
| `MPC-TST-007` | BLOCKING | 자동 | E2E가 CLI, Codex MCP, Pi, migration, purge를 포함한다. | §5 시나리오 report | 모든 필수 E2E PASS | 하나 미실행 |
| `MPC-TST-008` | BLOCKING | 자동 | release evidence가 동일 release commit과 clean tree를 가리킨다. | evidence manifest | commit/hash 일치 | mixed commit·dirty tree |
| `MPC-TST-009` | BLOCKING | 혼합 | 증거 archive 위치, 형식, 보존 기간, 변경 방지 정책이 확정되고 적용된다. | `MPC-TDF-009` 해소 기록 | raw logs·hash·서명 보존 | evidence 위치·retention 미정 |
| `MPC-TST-010` | BLOCKING | 자동 | `VB-RELEASE` 전체가 최종 release commit에서 마지막으로 실행됐다. | timestamped raw logs | 모든 exit 0 또는 인정된 baseline만 존재 | 일부 command 미실행 |
| `MPC-TST-011` | BLOCKING | 수동 | 수동 검토 결과가 항목별로 서명되고 release commit에 결박된다. | `ER-MANUAL` | 누락 서명 0 | unsigned review |
| `MPC-TST-012` | BLOCKING | 혼합 | Release Decision Record가 모든 check ID와 증거 URI/hash를 참조한다. | completed record | orphan check/evidence 0 | 요약 판정만 존재 |

### 3.15 비차단 후속 개선 경계

| ID | 등급 | 방식 | 완료 조건 | 필수 증거/명령 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-INF-001` | INFORMATIONAL | 수동 | 계획 §27의 시각적 Markdown, 추가 export, model metadata, 외부 웹 freshness, encrypted store, 수치 평가, multi-project, 전용 UI는 v1 완료 조건이 아님을 Release Decision Record에 명시한다. | scope review | 후속 개선과 v1 필수 범위가 분리됨 | 후속 개선 미구현을 v1 실패로 오분류하거나 v1 기능으로 과장 |
| `MPC-INF-002` | INFORMATIONAL | 혼합 | 후속 개선 일부가 우연히 포함됐더라도 v1 invariant, 보안, adapter 경계를 우회하지 않으며 별도 승인 범위로 추적된다. | diff·scope review | 별도 check와 근거가 있고 기존 BLOCKING gate 전부 통과 | 실험 기능이 hidden state·예외 경로를 만듦 |
| `MPC-INF-003` | INFORMATIONAL | 자동 | 저장소 규칙이 바뀌기 전에는 planning TUI가 후속 후보나 runtime dependency에도 포함되지 않는다. | `MPC-SCP-006`, `MPC-NEG-011`, dependency scan | planning TUI 관련 production path 0 | 후속 개선 명목의 planning ratatui surface 존재 |

---

## 4. 삭제되었음을 증명하는 Negative-Proof Checklist

정적 검색 하나만 통과해서는 삭제 완료로 판정하지 않는다. 각 항목은 다음 세 관점 중 명시된 관점을 모두 통과해야 한다.

1. **Source proof:** runtime source·dependency·projection 정의가 없음
2. **Install proof:** clean install/update 결과에 파일·등록이 없음
3. **Runtime proof:** 제거된 command·hook·workflow가 실행되지 않거나 존재하지 않음

| ID | 등급 | 방식 | 삭제 완료 조건 | 필수 증거 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-NEG-001` | BLOCKING | 자동 | legacy managed path inventory가 Slice 0 fixture와 일치한다. | baseline fixture·current tree | 모든 대상 분류 | 미분류 legacy path |
| `MPC-NEG-002` | BLOCKING | 혼합 | Team·Ultragoal CLI와 Rust runtime이 없다. | source, CLI parser snapshot, runtime invocation | source/install/runtime 3관점 부재 | 이름만 숨기고 command/runtime 잔존 |
| `MPC-NEG-003` | BLOCKING | 혼합 | deep-interview·ralplan·team·ultragoal Skill과 fragment가 clean install에 없다. | `rg`, install tree, adapter E2E | migration fixture/docs 외 0 | harness 또는 install output 잔존 |
| `MPC-NEG-004` | BLOCKING | 혼합 | managed Codex `hooks.json` projection 전체와 lifecycle hook 등록이 없다. | clean config/tree, source scan | hook entry 0 | SessionStart 포함 hook 잔존 |
| `MPC-NEG-005` | BLOCKING | 혼합 | PreToolUse, PostToolUse, Stop, SubagentStart, SubagentStop runtime이 없다. | `VB-MIGRATION-NEGATIVE`, runtime event attempt | migration fixture/docs 외 0 | event handler·projection 존재 |
| `MPC-NEG-006` | BLOCKING | 자동 | git guard와 mutation guard가 planning runtime에 없다. | source/dependency graph | import·registration 0 | 별도 이름으로 재연결 |
| `MPC-NEG-007` | BLOCKING | 자동 | hidden workflow metadata parser가 없다. | malformed metadata fixture·source scan | metadata가 state를 바꾸지 않음 | hidden marker로 transition |
| `MPC-NEG-008` | BLOCKING | 자동 | execution retry·continuation code가 없다. | source/process trace | automatic retry 0 | 실패 후 hidden 재실행 |
| `MPC-NEG-009` | BLOCKING | 자동 | commit·execution·subagent·goal receipt state와 validator가 없다. | schema/event/source inventory | 관련 entity/event/table 0 | receipt 요구·저장 |
| `MPC-NEG-010` | BLOCKING | 자동 | Pi role/subagent/fallback/retry 코드가 제거됐다. | Pi source·runtime tools snapshot | 등록·process 0 | 명칭 변경 후 잔존 |
| `MPC-NEG-011` | BLOCKING | 자동 | planning ratatui module·screen·dependency edge가 없다. | source/dependency scan | install/update/doctor 외 import 0 | planning TUI 존재 |
| `MPC-NEG-012` | BLOCKING | 자동 | daemon, queue, polling, auth server, issue broker, worktree manager, dashboard가 추가되지 않았다. | Cargo/module/process inventory | 관련 runtime 0 | 하나라도 존재 |
| `MPC-NEG-013` | BLOCKING | 자동 | adapter가 `rusqlite` 또는 planning DB path에 직접 접근하지 않는다. | source scan·file monitor | 직접 access 0 | 직접 read/write |
| `MPC-NEG-014` | BLOCKING | 자동 | removed command 이름이 CLI에서 숨겨졌을 뿐 parser/runtime에 남아 있지 않다. | parser test·직접 invocation | command 미인식, side effect 0 | hidden/deprecated alias로 실행 |
| `MPC-NEG-015` | BLOCKING | 혼합 | clean install 및 migrated existing install 모두 legacy file·hook을 남기지 않는다. | 두 설치 tree diff | 허용 migration backup 외 0 | runtime 위치에 residue |
| `MPC-NEG-016` | BLOCKING | 자동 | legacy old state path에 신규 planning operation이 write하지 않는다. | filesystem monitor | `.megara/state/workflows`, legacy artifacts write 0 | 신규 state 생성 |
| `MPC-NEG-017` | BLOCKING | 자동 | legacy artifact import가 승인·current spec·plan을 자동 생성하지 않는다. | migration DB inspection | Interview+DeltaAudit만 존재 | approved/candidate 자동 생성 |
| `MPC-NEG-018` | BLOCKING | 혼합 | 삭제 증거가 grep 결과에만 의존하지 않는다. | source+install+runtime evidence manifest | 각 삭제 항목에 2개 이상 독립 증거 | `rg` 로그만 제출 |

---

## 5. 필수 E2E 시나리오

각 E2E는 독립적인 임시 project에서 시작한다. 각 시나리오는 release commit, project tree 전후 hash, DB/event dump, command 원문, exit code를 보존해야 한다.

| ID | 등급 | 방식 | 시나리오와 핵심 절차 | 필수 증거 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- | --- |
| `MPC-E2E-001` | BLOCKING | 자동 | **Clean project CLI journey:** start→evidence refresh→delta audit→question→answer→full audit→spec generate→CLI approve→plan generate→CLI approve→bundle export | 전체 request/response, event sequence, artifacts | Complete, event/revision 계약, export privacy 통과 | hidden 실행, 누락 transition, raw transcript 노출 |
| `MPC-E2E-002` | BLOCKING | 혼합 | **Clean project Codex journey:** MCP 설치→initialize→질문 표시→typed proposal→host-prompt spec/plan approve | MCP trace, host screenshot, normalized state | host prompt 후 Complete | prompt 없이 승인·adapter rewrite |
| `MPC-E2E-003` | BLOCKING | 혼합 | **Clean project Pi journey:** extension install→one-shot RPC→QuestionProjection→slash approval | RPC logs, UI capture, child process trace | 현재 model 유지, child 0, Complete | tool approve·background process |
| `MPC-E2E-004` | BLOCKING | 자동 | **다중 session 선택:** 0개, open 1개, open 여러 개, complete 혼합에서 read fallback과 mutation explicit session 검증 | CLI matrix | §16.3 정확히 일치 | 임의 active 선택 |
| `MPC-E2E-005` | BLOCKING | 자동 | **Stale client:** 두 writer가 같은 revision으로 mutation 제출 | DB/event trace | 하나만 성공, 다른 요청 conflict/busy | lost update |
| `MPC-E2E-006` | BLOCKING | 자동 | **Cross-transport idempotency:** 같은 command ID/request를 CLI 후 Pi 또는 MCP로 재전송 | request hash, event count | event 1개, replay true | transport별 event 생성 |
| `MPC-E2E-007` | BLOCKING | 자동 | **Repo mutation before approval:** candidate 생성 후 HEAD/status/cited file 변경, show/status/approve/export 실행 | health·error traces | query warning, sensitive operation 차단 | stale 승인·bundle 생성 |
| `MPC-E2E-008` | BLOCKING | 자동 | **Evidence refresh invalidation:** approved spec 또는 plan 후 evidence 변경·refresh | event effects, state dump | Interview 복귀, artifact revoke | approval 잔존 |
| `MPC-E2E-009` | BLOCKING | 자동 | **Projection conflict:** candidate commit 후 Markdown 수동 변경, 동일 command replay protect→force | DB/event/file hashes | DB mutation 1회, protect 후 force projection만 변경 | event 중복·manual edit import |
| `MPC-E2E-010` | BLOCKING | 자동 | **Interrupted migration resume:** 각 journal step과 DB commit/manifest 사이에서 종료 | manifest·DB·FS trace | 중복 session 0, applied 완료 | duplicate/import loss |
| `MPC-E2E-011` | BLOCKING | 자동 | **Interrupted migration rollback:** prepared, planning_imported, projection_removed에서 rollback | byte hash comparison | 원본 byte-equivalent | residue·state 손실 |
| `MPC-E2E-012` | BLOCKING | 자동 | **User-modified legacy file:** apply/update를 기본 모드로 실행 | sentinel hash, warning | 파일 보존 | 삭제·overwrite |
| `MPC-E2E-013` | BLOCKING | 자동 | **Purge retry:** session purge 후 같은 purge command와 과거 command를 재전송 | receipt/tombstone rows | purge replay, retired error | session 부활·ID 재사용 |
| `MPC-E2E-014` | BLOCKING | 자동 | **Purge cleanup failure:** artifact/backup delete 또는 VACUUM 실패 후 doctor repair | pending receipt, residue scan | logical purge 유지, repair 후 clean | 성공 오보고·민감 residue |
| `MPC-E2E-015` | BLOCKING | 자동 | **DB corruption/cache divergence:** corrupted DB와 state cache 불일치를 각각 진단 | doctor/error logs | DB_CORRUPT와 PROJECTION_DIVERGED 구분 | panic·silent reset |
| `MPC-E2E-016` | BLOCKING | 자동 | **Schema mismatch:** 낮은·높은 user_version으로 일반 command 실행 | typed errors | upgrade required/unsupported 정확 | 자동 migration |
| `MPC-E2E-017` | BLOCKING | 자동 | **Git identity matrix:** clean, dirty, detached, unborn, non-Git에서 동일 planning journey의 evidence 단계 실행 | snapshot dump | 모든 지원 상태 성공 | 허용 상태 실패 |
| `MPC-E2E-018` | BLOCKING | 자동 | **악성 입력:** 4MiB 초과, 10,001 operations, deep nesting, unknown enum, path escape | process/state logs | panic 0, event/revision +0 | hang·partial state |
| `MPC-E2E-019` | BLOCKING | 혼합 | **Existing Codex config:** comments·다른 servers·unmanaged same-name table이 있는 project install/update/remove | config byte diff, backups | managed table만 변경, conflict 보호 | 전체 재직렬화·unmanaged overwrite |
| `MPC-E2E-020` | BLOCKING | 자동 | **Clean install negative proof:** fresh project 설치 후 legacy Skill·hook·TUI·runtime file 부재와 planning E2E 성공 | install tree, runtime trace | legacy 0, planning 정상 | 삭제 때문에 planning 불능 또는 legacy 잔존 |

---

## 6. 수동 UX·보안·복구 검토

모든 항목은 release commit에 결박된 서명 기록이 필요하다.

| ID | 등급 | 완료 조건 | 필수 증거 | PASS | FAIL |
| --- | --- | --- | --- | --- | --- |
| `MPC-MAN-001` | MANUAL | question-authoring gold fixture가 일곱 rubric을 모두 충족한다. | 항목별 yes/no와 인용 문구 | 7/7 yes | 하나라도 no·미서명 |
| `MPC-MAN-002` | MANUAL | jargon anti-example이 왜 실패하는지 rubric ID별로 설명된다. | anti-example review | 모든 결함이 규칙에 연결 | “어렵다” 같은 총평만 존재 |
| `MPC-MAN-003` | MANUAL | 각 choice의 direction, benefit, tradeoff가 서로 다른 정보를 제공한다. | gold fixture field review | 반복·동의어 대체 없음 | label을 장문으로 반복 |
| `MPC-MAN-004` | MANUAL | recommendation이 source 내용과 실제로 부합한다. | source ref와 추천 이유 대조 | 근거가 선택을 지지 | 존재하는 source ID만 형식적으로 연결 |
| `MPC-MAN-005` | MANUAL | Codex approval·purge prompt가 실제 host에서 사용자 confirmation을 요구한다. | host screen recording/log | 확인 전 event 0, 확인 후 1 | 자동 실행 |
| `MPC-MAN-006` | MANUAL | Pi approval·purge UI가 대상 ID/hash/revision을 사람이 확인할 수 있게 표시한다. | screenshot와 실행 command 비교 | 표시값=실행값 | 대상 식별 불가 |
| `MPC-MAN-007` | MANUAL | migration dry-run report가 삭제·보존·user-modified·backup 범위를 운영자가 구분할 수 있게 표시한다. | dry-run output review | 각 범주 명확 | 어떤 파일이 삭제될지 불명 |
| `MPC-MAN-008` | MANUAL | 문서·CLI가 구조적 보증과 의미적 품질 비보증을 일관되게 설명한다. | docs/help/error text review | 과장 표현 0 | “품질 보증”, “구현 가능성 보증” |
| `MPC-MAN-009` | MANUAL | doctor의 corruption, divergence, pending purge residue 설명이 복구 행동을 명확히 제시한다. | 실제 오류 화면·복구 수행 | 문서만으로 복구 성공 | 숨은 명령·상태 손실 |
| `MPC-MAN-010` | MANUAL | user-modified Codex config와 legacy file 충돌 안내가 기본 보호와 `--force` 위험을 설명한다. | conflict output review | backup·보호·영향 표시 | force만 권고하거나 silent skip |
| `MPC-MAN-011` | MANUAL | purge 문서가 app-level 삭제 범위와 SSD·외부 backup 비보증을 구분한다. | security docs review | 두 범위 명확 | forensic erase 보장 주장 |
| `MPC-MAN-012` | MANUAL | export 기본값이 transcript를 포함하지 않는다는 사실과 `--include-transcript` 위험이 명확하다. | export help/manifest review | 민감정보 경고 존재 | 옵션 영향 불명 |
| `MPC-MAN-013` | MANUAL | planning flow가 초심자에게 구현 실행과 기획 완료를 혼동시키지 않는다. | CLI/Codex/Pi 완료 화면 review | “계획 승인 완료” 수준으로 표현 | 구현·테스트 완료처럼 표현 |
| `MPC-MAN-014` | MANUAL | rollback 운영자가 backup, session 변경 충돌, 이전 release 재설치 단계를 재현할 수 있다. | 독립 검토자 복구 수행 기록 | 추가 설명 없이 성공 | 개발자 구두 지원 필요 |

---

## 7. 최종 Release Decision Record 템플릿

~~~
# Megara Planning Core v1 Release Decision Record

## 1. Release 식별

- Release version:
- Release commit SHA:
- Working tree clean: YES / NO
- Cargo.lock SHA-256:
- rustc -Vv:
- cargo -V:
- Target OS/architecture:
- Codex host version:
- Pi host version:
- Canonical plan revision/hash:
- Completion checklist revision/hash:
- Evidence archive URI:
- Evidence archive SHA-256:

## 2. Baseline

- Base commit SHA:
- Base Cargo.lock SHA-256:
- Baseline bundle URI:
- Baseline bundle SHA-256:
- 알려진 baseline failure 수:
- 각 baseline failure 재현 여부:
- 신규 failure 수:
- Baseline 판정: PASS / FAIL

| Failure ID | Base 재현 | Release 재현 | 동일 diagnostic 근거 | Planning 관련 여부 | 판정 |
| --- | --- | --- | --- | --- | --- |

## 3. Verification bundle

| Bundle | 실행 일시 | Exit code | 실행 test 수 | 실패 | ignored/skipped | 증거 URI/hash | 판정 |
| --- | --- | ---: | ---: | ---: | ---: | --- | --- |
| VB-BASELINE | | | | | | | |
| VB-DOMAIN | | | | | | | |
| VB-STORE-PROTOCOL | | | | | | | |
| VB-EVIDENCE-INTERVIEW | | | | | | | |
| VB-ARTIFACT | | | | | | | |
| VB-ADAPTER | | | | | | | |
| VB-MIGRATION-NEGATIVE | | | | | | | |
| VB-RELEASE | | | | | | | |

## 4. 체크 결과 집계

| 등급 | 전체 | PASS | FAIL | UNVERIFIED | N/A |
| --- | ---: | ---: | ---: | ---: | ---: |
| BLOCKING | | | | | |
| MANUAL | | | | | |
| INFORMATIONAL | | | | | |

## 5. 실패 또는 미검증 항목

| Check ID | 상태 | 증거 | 영향 | 담당자 |
| --- | --- | --- | --- | --- |
| `MPC-EVD-008` | `PASS` | `test/unit/planning_service_health.rs::stale_full_audit_checks_revision_before_evidence_or_proposal_shape`; `test/unit/planning_service_health.rs::delta_audit_remains_available_when_cited_file_is_missing`; `test/unit/planning_artifact_evidence.rs::stale_evidence_blocks_spec_and_plan_generate_and_approve_with_zero_delta`; `test/unit/planning_export.rs::bundle_stale_and_missing_approval_are_blocked_before_filesystem_write` — FullAudit stale block, Delta stale bypass, spec/plan generate+approve zero-delta stale matrix, approved bundle export stale block | 없음 | Slice4 |

BLOCKING 또는 MANUAL 항목이 한 건이라도 있으면 최종 판정은 FAIL이다.

## 6. TO_DEFINE 해소

| TO_DEFINE ID | 확정 계약 | 승인 문서/commit | 대응 test ID | 판정 |
| --- | --- | --- | --- | --- |

## 7. Negative proof

| Check ID | Source proof | Install proof | Runtime proof | 판정 |
| --- | --- | --- | --- | --- |

## 8. E2E

| E2E ID | 환경 | 증거 URI/hash | 판정 |
| --- | --- | --- | --- |

## 9. Manual review

| Check ID | 검토자 | 역할 | 검토 일자 | 증거 URI/hash | 판정 | 서명 |
| --- | --- | --- | --- | --- | --- | --- |

## 10. 알려진 INFORMATIONAL 항목

| ID | 내용 | 사용자 영향 | 후속 관리 |
| --- | --- | --- | --- |

## 11. 최종 판정

- 최종 판정: PASS / FAIL
- 판정 근거:
- BLOCKING 실패·미검증 수:
- MANUAL 실패·미서명 수:
- 신규 failure 수:
- 미해결 TO_DEFINE 수:

## 12. Release authority

- 이름:
- 역할:
- 판정 일자:
- 서명:
~~~

---

## 8. 이 계획만으로 지금 확정 가능한 완료 체크

다음 항목은 canonical 계획에 판정 기준이 충분히 정의되어 있으므로 구현과 증거가 존재하면 추가 제품 결정을 거치지 않고 판정할 수 있다.

- 제품 범위와 비목표: `MPC-SCP-001`–`009`
- 상태 소유권: `MPC-OWN-001`–`007`
- lifecycle·revision·invalidation: `MPC-STM-001`–`013`
- event aggregate와 replay: `MPC-EVT-001`–`009`
- SQLite transaction·WAL·idempotency·schema error: `MPC-DB-001`–`011`
- CLI command tree와 session 선택: `MPC-CLI-001`–`010`
- 공통 envelope, request hash, typed error, 기본 resource limit: `MPC-PRO-001`–`012`
- Git/non-Git evidence freshness와 invalidation: `MPC-EVD-001`–`004`, `006`–`013`
- QuestionProposal 구조와 projection provenance: `MPC-QST-001`–`015`
- DeltaAudit·FullAudit·readiness: `MPC-AUD-001`–`008`
- spec candidate·hash·approval·revision: `MPC-SPC-001`–`010`
- plan traceability·validator·approval: `MPC-PLN-001`–`012`
- Markdown projection과 export: `MPC-PRJ-001`–`007`, `MPC-EXP-001`–`006`
- purge logical/physical 처리: `MPC-PUR-001`–`012`
- adapter의 기본 책임과 동등성: `MPC-ADP-001`–`013`
- Codex MCP 세부 계약: `MPC-MCP-001`–`008`
- Pi의 모델 경계와 RPC 책임: `MPC-PI-001`–`005`, `007`, `008`
- migration journal·resume·rollback·legacy import: `MPC-MIG-001`–`016`
- install/update/doctor 회귀와 파일 보호: `MPC-REG-001`–`010`
- negative proof: `MPC-NEG-001`–`018`
- baseline 구분과 test matrix 추적: `MPC-BAS-*`, `MPC-TST-001`–`008`, `010`–`012`
- 필수 E2E 중 TO_DEFINE에 직접 의존하지 않는 시나리오
- 수동 UX·보안·복구 항목

---

## 9. 완료 판정을 위해 추가로 확정해야 하는 TO_DEFINE

아래 항목은 canonical 계획만으로 객관적인 PASS 기준을 만들 수 없으며, release 판정 시 하나라도 미해결이면 최종 완료 판정은 `FAIL`이다.

| ID | 등급 | 미확정 계약 | 완료 전에 필요한 확정 내용 | 차단되는 체크 |
| --- | --- | --- | --- | --- |
| `MPC-TDF-001` | BLOCKING | `project_id` 생성 알고리즘 | canonical project root에서 project identity를 생성·저장·비교하는 exact algorithm, relocation 처리, 기존 DB mismatch 정책 | `MPC-SCP-004`, `MPC-PRO-002`, cross-transport idempotency |
| `MPC-TDF-002` | BLOCKING | operation별 success `result` schema | §16.4의 모든 command/query에 대한 versioned request params·success result JSON schema와 golden | `MPC-PRO-013`, MCP/Pi conformance |
| `MPC-TDF-003` | BLOCKING | Codex/Pi managed projection 제거 entrypoint | 기존 installer 체계에서 remove/uninstall을 실행하는 정확한 명령, `--force` 의미, Codex/Pi별 제거 파일·보존 파일 목록 | `MPC-ADP-015`, `MPC-E2E-019` |
| `MPC-TDF-004` | BLOCKING | 지원 Codex/Pi version matrix | v1이 지원한다고 주장할 최소 host version, OS별 조합, host confirmation 검증 환경 | `MPC-ADP-009`, `MPC-ADP-010`, `MPC-ADP-014` |
| `MPC-TDF-005` | BLOCKING | secret·credential 경로 deny 계약 | `.env`, pem, key, credential “계열”의 exact filename·extension·case 규칙과 `.env.example` 예외 | `MPC-EVD-005`, evidence security E2E |
| `MPC-TDF-006` | BLOCKING | Pi child process timeout·종료 정책 | timeout 값, SIGTERM/SIGKILL 또는 플랫폼별 종료 순서, stderr·typed error mapping, orphan 방지 | `MPC-PI-006`, `MPC-RES-007` |
| `MPC-TDF-007` | BLOCKING | 성능·장시간 lock acceptance budget | 최소 fixture 규모와 start/status/replay/evidence refresh/migration/purge별 시간·메모리·lock 예산 | `MPC-RES-008`, release performance 판정 |
| `MPC-TDF-008` | BLOCKING | JSON nesting·parser resource 정책 | 허용 nesting depth 또는 동등한 parser 제한, 초과 시 typed error, stack/memory 안전 기준 | `MPC-RES-004`, `MPC-E2E-018` |
| `MPC-TDF-009` | BLOCKING | release evidence 보관 계약 | 증거 archive의 실제 위치 또는 시스템, manifest schema, 원문 로그 보존 기간, 접근 권한, 변경 방지·hash 검증 정책 | `MPC-TST-009`, 최종 Release Decision Record |
| `MPC-TDF-010` | BLOCKING | release 대상 플랫폼 | 지원 OS·architecture 목록과 각 플랫폼에서 필수로 실행할 install, path, SQLite, symlink, process termination 검증 범위 | 전체 release claim, `MPC-ADP-014`, `MPC-RES-*` |

`TO_DEFINE` 항목은 구현자가 임의의 상수나 동작을 선택하고 테스트를 그 값에 맞추는 방식으로 닫을 수 없다. 각 항목은 관련 구현을 완료 처리하기 전에 canonical 계획의 보충 계약으로 승인되고, 해당 계약을 직접 검증하는 fixture·golden·E2E와 함께 닫혀야 한다.
