---
type: Plan
title: Megara Planning Core v1 기획안
description: Megara를 모델 독립적인 로컬 기획 상태 엔진과 Codex/Pi 어댑터로 재설계하기 위한 개발 착수 문서.
timestamp: 2026-08-02
tags: [okf, plan, planning, megara, architecture]
---

# Megara Planning Core v1 기획안

## 0. 문서 지위

이 문서는 기존 Planning Core 재설계안을 전면 대체하는 v1 구현 기준이다. 제품 범위, 상태 소유권, 저장소, 명령, 승인, adapter, migration, 삭제 순서와 수용 기준을 확정한다.

개발 완료 판정은 [Planning Core v1 개발 완료 판정 체크리스트](planning-core-redesign-completion-checklist.md)를 사용한다. 구현이 존재한다는 주장만으로 완료 처리하지 않고, 같은 release commit에 결박된 자동 증거·수동 서명·negative proof·E2E 결과로 판정한다.

구현 중 이름이나 사용자 문구는 조정할 수 있다. 다음 계약은 변경하지 않는다.

- Megara는 기획 하네스다.
- 모델은 조사, 한 문항 질문, 의미 평가, spec·plan 후보 생성을 담당한다.
- Rust PlanningCore만 authoritative state, command 검증, 단계 전환, stale 전파, 승인 결박, 구조 검증을 소유한다.
- 모델 출력은 typed proposal이다. 모델 문장만으로 상태가 전환되지 않는다.
- 구현, 테스트 실행, 커밋, 배포, team, goal, mutation 통제는 범위 밖이다.
- planning workflow를 Skill, hook, hidden metadata로 구현하지 않는다.

## 1. 확정 결정

| 항목 | v1 결정 |
| --- | --- |
| 제품 | 프로젝트 요구사항 인터뷰, spec 결정화, plan 구조 검증을 수행하는 기획 하네스 |
| 상태 범위 | project-local only |
| 프로젝트 루트 | 명시적 --project, 생략 시 현재 작업 디렉터리. 상위 디렉터리 자동 탐색 없음 |
| authoritative store | 프로젝트당 단일 .megara/planning/planning.db |
| 저장 엔진 | SQLite WAL |
| 원본 | append-only semantic event log |
| 파생 상태 | event와 같은 transaction에서 갱신하는 재생성 가능한 state projection |
| 사용자 산출물 | DB에서 생성한 spec.md와 plan.md |
| Markdown 직접 편집 | v1 비지원 |
| 모델 실행 | Codex/Pi host에서 현재 선택된 모델과 추론 수준 사용 |
| Rust의 모델 호출 | 없음 |
| 질문 | 동시에 한 문항 |
| 준비도 | blocker와 구조 invariant 기반 |
| 숫자 모호성 점수 | v1에 없음 |
| 연속 통과 | v1에 없음 |
| full audit | spec 후보 생성 직전 현재 입력 전체에 대해 필수 |
| spec 승인 | candidate_id + semantic_hash + base_domain_revision 결박 |
| plan 승인 | candidate_id + semantic_hash + base_plan_revision 결박 |
| Codex adapter | project .codex/config.toml에 등록하는 MCP stdio server |
| Pi adapter | extension tool + user slash command + one-shot JSON RPC |
| planning TUI | 없음 |
| planning hook | 없음 |
| 역할 fan-out | 없음. planner·architect·critic 상태와 subagent gate를 만들지 않음 |
| replay | 모델, 네트워크, Git, 현재 파일 조사를 호출하지 않는 pure fold |
| purge | append-only의 유일한 예외. session 내용 물리 삭제 후 최소 tombstone 보존 |
| 보증 범위 | 최신성, 추적성, 참조 무결성, 승인 결박, 구조 완결성 |
| 비보증 범위 | 의미적 정확성, 구현 가능성, 기술 품질, 검증 방법의 실효성 |

## 2. 참고 구현에서 채택할 것

### 2.1 비교

| 대상 | 채택 | 거부 |
| --- | --- | --- |
| grill-me / grilling | 한 번에 한 질문, 의존 순서 탐색, 코드로 확인 가능한 사실은 조사하고 결정만 질문 | 자연어 합의만으로 완료, 모델 대화 기억을 상태로 사용 |
| Ouroboros interview | 질문 생성과 상태 관리 분리, 답변 원문 보존, closure audit, 명시적 승인 | LLM 점수 0.2, 차원별 floor, 2회 연속 통과, 실행 workflow 결합 |
| OMC deep-interview | intent 우선, brownfield 조사, non-goal·decision boundary, pressure review | profile별 숫자 threshold, round cap, ralplan/team/autopilot 전환 |

### 2.2 Megara식 해석

- grill-me는 질문 원칙이다. 설치 Skill이나 상태 owner가 아니다.
- Ouroboros의 영속성과 독립 재평가는 event store와 full audit로 옮긴다.
- Ouroboros의 수치 score는 모델 drift를 기계적 사실처럼 취급하므로 v1 gate에서 제거한다.
- OMC의 pressure pass는 full audit의 counterexample_review로 옮긴다.
- 세 구현의 실행 bridge, subagent fan-out, hidden mode state는 가져오지 않는다.

## 3. 현재 저장소 경계

다음 기존 책임을 유지한다.

- harness/는 내장 하네스 원본이다.
- src/templates.rs와 src/templates/specs.rs는 tracked harness 파일을 binary에 색인한다. planning domain logic을 넣지 않는다.
- src/cli.rs와 src/cli/는 clap 명령 정의를 소유한다.
- src/main.rs는 command dispatch를 소유한다.
- src/targets/codex.rs와 src/targets/codex/는 Codex projection을 소유한다.
- src/targets/pi.rs와 harness/pi/extensions/megara.ts는 Pi projection을 소유한다.
- src/installer/planner.rs는 runtime .gitignore와 managed projection 계획을 소유한다.
- src/installer/migration.rs는 기존 .agents/state에서 .megara/state로 이동하는 migration을 계속 소유한다.
- 기존 사용자 파일은 --force 없이 덮어쓰거나 삭제하지 않는다.
- ratatui는 install, update, doctor의 command-scoped adapter에서만 사용한다.

planning 구현을 위해 issue broker, daemon, queue, polling, auth server, worktree manager, dashboard를 추가하지 않는다.

## 4. 목표와 비목표

### 4.1 목표

- 중단된 인터뷰를 재개한다.
- 여러 planning session을 한 프로젝트에서 보존한다.
- 질문·답변·조사 근거·평가·승인을 추적한다.
- 모델 비결정성과 상태 replay를 분리한다.
- stale command와 중복 command를 안전하게 거부한다.
- spec·plan이 어떤 입력 revision에서 만들어졌는지 증명한다.
- 요구사항, 성공 조건, plan step, verification의 참조 무결성을 검사한다.
- Codex와 Pi가 같은 Rust command/query contract를 사용한다.
- legacy workflow를 자동 승인 없이 가져오고 되돌릴 수 있다.

### 4.2 비목표

- 소스 파일 변경
- 테스트 실행과 결과 판정
- 구현 단계 실행 또는 완료 추적
- commit, branch, pull request, release
- team 또는 subagent 배치
- ultragoal 또는 지속 목표
- commit·execution·subagent receipt
- PreToolUse, PostToolUse, Stop, SubagentStart, SubagentStop
- mutation guard와 git guard
- daemon, queue, polling, auth, issue broker, worktree, dashboard
- planning ratatui
- cloud sync와 사용자 전역 planning DB
- multi-project session
- 모델 선택, routing, fallback, reasoning level 변경
- hidden chain-of-thought 요청·저장
- Markdown을 canonical state로 역파싱
- 부분 transcript redaction
- 자연어 계획의 기술적 타당성 보증

비-workflow 유틸리티의 존폐는 이 계획이 결정하지 않는다. 단, planning lifecycle은 어떤 Skill에도 의존하지 않는다.

## 5. 소유권과 실행 흐름

~~~text
Codex current model                    Pi current model
  │ built-in repo tools                  │ built-in repo tools
  │ typed proposal                       │ typed proposal
  ▼                                      ▼
Codex MCP stdio adapter               Pi extension adapter
  └──────────────────┬───────────────────┘
                     │ CommandPort / QueryPort
                     ▼
                Rust PlanningCore
       validate · transition · invalidate · approve
                     │
                     ▼
        .megara/planning/planning.db
                     │
                     ▼
         generated spec.md / plan.md
~~~

| 주체 | 허용 | 금지 |
| --- | --- | --- |
| PlanningCore | command 검증, event append, reducer, transition, hash, invalidation, structural validation | LLM 호출, UI 선택, 구현 실행 |
| Host model | evidence 후보, QuestionProposal, AuditProposal, SpecProposal, PlanProposal | DB write, phase 지정, 승인 event 생성 |
| 사용자 | answer, revise, exact candidate 승인, export, purge | 없음 |
| Adapter | host tool/UI와 core port 중계, actor 확정 | DB·artifact 직접 write, stale 계산 |
| Projection | 사람이 읽는 표현 | authoritative input |

PlanningCore는 동기 Rust library다. MCP와 Pi adapter만 transport와 host API를 안다.

## 6. 프로젝트와 파일 경계

### 6.1 프로젝트 루트

모든 planning 명령은 다음 규칙을 사용한다.

1. --project PATH가 있으면 PATH를 canonicalize한다.
2. 없으면 현재 작업 디렉터리를 canonicalize한다.
3. 상위 .megara 또는 Git root를 자동 검색하지 않는다.
4. 상태와 artifact는 확정된 root 아래에만 쓴다.

이 규칙은 현재 project-scope installer가 cwd를 기준으로 하는 동작과 일치하며, 다른 repository 상태를 실수로 여는 것을 막는다.

### 6.2 저장 구조

~~~text
<project>/
  .megara/
    .gitignore
    planning/
      planning.db
      planning.db-wal
      planning.db-shm
      artifacts/
        <session-id>/
          spec.md
          plan.md
          projection-manifest.json
    migration-backups/
      <migration-id>/
        manifest.json
        legacy-state/
        legacy-artifacts/
        managed-projection/
~~~

### 6.3 Git ignore

harness/.gitignore와 src/installer/planner.rs의 runtime .gitignore 생성값을 모두 다음으로 바꾼다.

~~~gitignore
state/
artifacts/
cache/
planning/
migration-backups/
~~~

planning/ 전체를 무시한다. DB와 WAL에는 답변 원문, proposal, evidence path가 포함될 수 있다. Git에 공유할 spec과 plan은 planning export로 명시적으로 내보낸다.

## 7. SQLite 계약

### 7.1 의존성과 설정

Cargo에 다음 기능을 추가하고 Cargo.lock으로 고정한다.

- rusqlite: bundled SQLite
- uuid: UUIDv7
- unicode-normalization: semantic hash 정규화
- toml_edit: 기존 Codex project config의 key와 comment를 보존하는 managed table merge
- rmcp: MCP server와 stdio transport feature
- tokio: MCP adapter에 필요한 current-thread runtime과 I/O feature만 사용

DB open 시 다음 runtime PRAGMA를 적용한다.

~~~sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA secure_delete = ON;
~~~

- user_version은 먼저 읽는다.
- 새 DB를 schema v1로 생성하거나 명시적 schema migration을 commit할 때만 user_version=1을 설정한다.
- binary보다 DB schema가 낮으면 일반 command가 자동 migration하지 않고 SCHEMA_UPGRADE_REQUIRED를 반환한다.
- schema upgrade는 backup을 만드는 update 또는 doctor --repair에서만 실행한다.
- binary보다 높은 schema는 SCHEMA_VERSION_UNSUPPORTED로 거부한다.

### 7.2 최소 schema

| Table | 목적 | 핵심 column |
| --- | --- | --- |
| project_meta | schema와 project identity | key, value_json |
| sessions | 재생성 가능한 current projection | session_id, phase, revision, domain_revision, plan_revision, state_json, normalized_state_hash |
| events | authoritative append-only log | event_id, session_id, seq, event_type, semantic_payload_json, metadata_json, state_hash_after |
| command_results | idempotency core result cache | command_id, session_id, request_hash, core_response_json, resulting_revision |
| purged_sessions | purge result와 최소 tombstone | session_id, purged_at, purge_schema_version, purge_command_id, request_hash, core_response_json, cleanup_state, pending_backup_id |
| purged_command_ids | purge 뒤 command ID 재사용 차단 | command_id, session_id |

필수 제약:

- events는 UNIQUE(session_id, seq)를 가진다.
- 새 command_id는 command_results, purged_sessions.purge_command_id, purged_command_ids를 모두 조회해 프로젝트 DB 전체에서 unique하게 취급한다.
- sessions.state_json은 cache다. events가 원본이다.
- v1은 entity별 relational projection table을 만들지 않는다.

### 7.3 Mutation transaction

~~~text
BEGIN IMMEDIATE
  → command_id 조회
  → 동일 ID이면 request hash 비교
  → session과 expected_revision 확인
  → event fold 결과 또는 current cache load
  → command invariant 검증
  → 정확히 하나의 aggregate semantic event append
  → state projection과 hash 갱신
  → authoritative core result 저장
COMMIT
~~~

- writer는 BEGIN IMMEDIATE로 직렬화한다.
- 5초 안에 writer lock을 얻지 못하면 DB_BUSY를 반환한다.
- commit 전 crash는 command 전체를 rollback한다.
- commit 후 event, state projection, core result는 함께 존재한다.
- 한 성공 mutation은 event 한 개를 append하며 session seq와 revision은 같은 값이다.
- invalidation과 approval revoke는 별도 event가 아니라 해당 aggregate event의 effects에 포함한다.
- Markdown sync는 DB commit 뒤 수행하는 비-authoritative 후처리다.
- transport response는 저장된 core result와 매 호출 시 관찰한 projection_status를 분리한다.
- projection_status는 written, unchanged, conflict, io_error 중 하나다.
- conflict와 io_error는 성공한 DB mutation을 실패로 바꾸지 않고 warning으로 반환한다.
- 동일 command 재호출은 event를 만들지 않고 저장된 core result를 replay한 뒤 projection sync만 안전하게 재시도할 수 있다.

### 7.4 Replay

~~~text
events ORDER BY seq
  → versioned Rust reducer
  → PlanningState
  → normalized_state_hash
~~~

replay 중 모델, 네트워크, Git, 파일 조사, Markdown을 호출하지 않는다.

sessions.state_json 또는 normalized_state_hash가 replay 결과와 다르면 PROJECTION_DIVERGED다. doctor는 read-only 진단을 제공하고, doctor --repair만 cache와 Markdown을 재생성한다. event 자체를 자동 수정하지 않는다.

## 8. 상태 모델

### 8.1 Lifecycle과 직교 상태

~~~rust
pub enum LifecyclePhase {
    Interview,
    Specification,
    Planning,
    Complete,
}

pub struct PlanningState {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub revision: u64,
    pub domain_revision: u64,
    pub plan_revision: u64,
    pub phase: LifecyclePhase,
    pub pending_question: Option<PendingQuestion>,
    pub required_model_action: Option<ModelWorkItem>,
    pub blockers: BTreeMap<BlockerId, Blocker>,
    pub imported_legacy_context: bool,
    pub entities: EntityGraph,
    pub transcript: TranscriptIndex,
    pub repo_snapshot: Option<RepoEvidenceSnapshot>,
    pub full_audit: Option<FullAuditRef>,
    pub spec: ArtifactTrack<SpecCandidate>,
    pub plan: ArtifactTrack<PlanCandidate>,
}

pub struct ArtifactTrack<T> {
    pub current_candidate: Option<T>,
    pub approval: Option<ApprovalRef>,
}

pub struct ObservedHealth {
    pub evidence_current: bool,
    pub projection_status: ProjectionStatus,
    pub residue_warnings: Vec<HealthWarning>,
}
~~~

waiting, blocked, approved, stale는 phase가 아니다. 다음처럼 계산한다.

~~~text
waiting_for_user = pending_question exists
waiting_for_model = required_model_action exists
blocked = blocking blocker exists
waiting_for_spec_approval =
  phase == Specification and current spec candidate exists and approval absent
waiting_for_plan_approval =
  phase == Planning and current plan candidate exists and approval absent
spec_stale = candidate stale or approval base_domain_revision mismatch
plan_stale = candidate stale or approval base_plan_revision mismatch
~~~

ObservedHealth는 현재 Git·filesystem을 읽는 query 결과이며 event-sourced PlanningState가 아니다. replay와 normalized_state_hash에 포함하지 않는다. 외부 파일 변화만으로 canonical state를 몰래 변경하지 않는다.

### 8.2 Revision 의미

- revision: 성공한 mutation마다 정확히 1 증가한다.
- domain_revision: initial request, answer, evidence, spec-domain entity, blocker의 의미가 바뀔 때 증가한다.
- plan_revision: approved spec 기준 또는 plan revision feedback이 바뀔 때 증가한다.
- query와 idempotent replay는 revision을 바꾸지 않는다.
- metadata-only 변화는 domain_revision을 바꾸지 않는다.

### 8.3 핵심 invariant

1. pending_question과 required_model_action은 동시에 존재할 수 없다.
2. pending_question은 최대 하나다.
3. answer는 정확한 question_id와 based_on_revision을 포함한다.
4. start 외 mutation은 session_id와 expected_revision이 필수다.
5. 모델은 phase, approval, canonical ID를 직접 지정하지 않는다.
6. domain_revision이 증가하면 full audit, spec candidate·approval, plan candidate·approval을 stale 처리한다.
7. plan_revision이 증가하면 plan candidate·approval만 stale 처리한다.
8. plan candidate는 current approved spec 없이는 만들 수 없다.
9. Complete는 current spec approval과 current plan approval이 모두 있을 때만 가능하다.
10. session purge 외에는 event row를 수정하거나 삭제하지 않는다.
11. 성공 mutation 하나는 aggregate event 하나와 revision 하나를 만든다.

### 8.4 Transition

| Current | Command | 필수 조건 | Next | 효과 |
| --- | --- | --- | --- | --- |
| 없음 | start | DB 정상 | Interview | initial request, DeltaAudit work item |
| 모든 phase | evidence refresh | path와 snapshot 유효 | 현재 phase 또는 Interview | 동일 snapshot이면 phase 유지. 변화가 있으면 invalidation과 approval revoke 후 Interview |
| Interview | audit apply delta | work item·hash 일치 | Interview | entity/blocker 갱신, 질문 또는 FullAudit work item |
| Interview | answer | pending question 일치 | Interview | raw answer 저장, DeltaAudit work item |
| Interview | audit apply full | current input hash 일치 | Specification 또는 Interview | ready면 phase 전환, 변경·blocker가 있으면 Interview 유지 |
| Specification | spec generate | current full audit ready | Specification | spec candidate 생성 |
| Specification | spec approve | exact binding, blocker 없음 | Planning | approval 기록, plan_revision 증가 |
| Specification | spec revise | candidate current | Interview | revision request, 모든 artifact stale |
| Planning | plan generate | approved spec current | Planning | plan candidate와 validator finding 생성 |
| Planning | plan approve | exact binding, structural blocker 없음 | Complete | approval 기록 |
| Planning | plan revise | candidate current | Planning | feedback 저장, plan candidate stale |
| Complete | spec revise | approved spec 지정 | Interview | spec·plan approval revoke |
| 모든 phase | purge | user confirmation | 삭제 | content 삭제, tombstone |

status, current, spec show, plan show는 ObservedHealth를 함께 계산한다. evidence가 stale이면 current는 evidence_refresh를 다음 action으로 표시한다. approved artifact의 bundle export는 EVIDENCE_STALE로 차단한다. state-json과 events-jsonl recovery export는 stale 상태에서도 허용한다.

## 9. Canonical entity와 provenance

### 9.1 Entity

| Entity | 필수 필드 |
| --- | --- |
| Problem | statement |
| Outcome | statement, observable_result |
| Fact | statement, evidence_refs |
| Decision | statement, selected_option |
| DecisionBoundary | autonomous_scope, requires_user_approval |
| Requirement | statement, priority |
| AcceptanceCriterion | statement |
| Constraint | statement |
| NonGoal | statement |
| Assumption | statement, validation_status |
| Risk | statement, impact, mitigation |
| PlanStep | objective, change_surface, rollback_or_recovery |
| Verification | method, procedure, expected_result |

Exact body contract:

- Problem: statement String.
- Outcome: statement String, observable_result String.
- Fact: statement String, evidence_refs non-empty EvidenceId array.
- Decision: statement String, selected_option String.
- DecisionBoundary: autonomous_scope non-empty String array, requires_user_approval String array.
- Requirement: statement String, priority must|should|could.
- AcceptanceCriterion: statement String.
- Constraint: statement String.
- NonGoal: statement String.
- Assumption: statement String, validation_status unverified|confirmed|rejected.
- Risk: statement String, impact low|medium|high|critical, mitigation String.
- PlanStep: objective String, change_surface non-empty path/module String array, rollback_or_recovery String.
- Verification: method command|assertion|metric|manual, procedure String, expected_result String.

AuditProposal의 EntityOp는 Problem부터 Risk까지만 만들 수 있다. PlanStep과 Verification은 approved spec을 입력으로 받는 PlanProposal에서만 만든다.

각 entity는 stable human ID, internal UUID, revision, disposition, validity, source_refs를 가진다.

disposition은 current, superseded, rejected 중 하나이며 core만 설정한다.

~~~rust
pub enum EntityValidity {
    Valid,
    Stale {
        since_domain_revision: u64,
        causes: Vec<SourceRef>,
    },
}
~~~

current entity는 disposition=Current와 validity=Valid를 모두 만족하는 revision이다.

- evidence 변화는 직접 참조 Fact를 Stale로 만든다.
- derived_from 또는 depends_on으로 도달 가능한 current entity에 Stale을 전파한다.
- stale entity에 연결된 edge는 보존하지만 readiness, FullAudit ready, spec·plan candidate에서 current ref로 인정하지 않는다.
- stale을 해제하는 in-place operation은 없다.
- 모델은 current evidence 또는 user source로 entity를 revise해 새 Valid revision을 만들어야 한다.
- stale cause와 전파 결과는 RepoEvidenceRefreshed aggregate event effects에 기록한다.

~~~text
PROB-001 OUT-001 FACT-001 DEC-001 DBND-001
REQ-001 AC-001 CON-001 NG-001 ASM-001 RISK-001
STEP-001 VER-001
~~~

모델 create proposal은 temp_ref를 사용한다. core가 canonical ID를 할당한다.

Entity 수정은 in-place overwrite가 아니다.

~~~text
REQ-001 rev2 --supersedes--> REQ-001 rev1
~~~

Candidate는 entity_id와 entity_revision을 함께 참조한다.

Blocker contract:

~~~rust
pub enum BlockerKind {
    MissingProblem,
    MissingOutcome,
    MissingRequirement,
    MissingNonGoal,
    MissingDecisionBoundary,
    MissingAcceptanceCriterion,
    OpenDecision,
    Contradiction,
    EvidenceRequired,
    InvalidSource,
    ModelOutputInvalid,
    ManualReviewRequired,
}

pub enum BlockerSeverity {
    Blocking,
    Advisory,
}
~~~

Blocker는 stable ID, revision, kind, severity, statement, non-empty source_refs, resolved_at_revision을 가진다. 삭제하지 않고 resolve revision을 만든다.

### 9.2 Edge 방향

문서와 코드에서 다음 방향만 사용한다.

~~~text
Requirement --has_acceptance_criterion--> AcceptanceCriterion
PlanStep --implements--> Requirement
Verification --verifies--> AcceptanceCriterion
Verification --executed_by--> PlanStep
PlanStep --depends_on--> PlanStep
Entity --depends_on--> Entity
Entity --derived_from--> SourceRef
new Entity revision --supersedes--> old Entity revision
Entity --conflicts_with--> Entity
~~~

derived_from은 출처만 뜻한다.

허용 예:

- Fact → RepoEvidence
- Requirement → InitialRequest 또는 UserAnswer
- Decision → UserAnswer
- PlanStep → ApprovedSpecCandidate

금지 예:

- Requirement → AcceptanceCriterion
- PlanStep → Requirement
- Verification → AcceptanceCriterion

금지 예는 각각 has_acceptance_criterion, implements, verifies를 사용한다.

### 9.3 Source 규칙

- Fact는 RepoEvidence가 필수다.
- Decision과 DecisionBoundary는 InitialRequest 또는 UserAnswer가 필수다.
- Requirement와 NonGoal은 InitialRequest, UserAnswer 또는 current Decision이 필수다.
- AcceptanceCriterion은 normative user source를 가지며 Requirement와 edge로 연결한다.
- PlanStep은 ApprovedSpecCandidate와 Requirement를 참조한다.
- Verification은 ApprovedSpecCandidate와 AcceptanceCriterion을 참조한다.
- repo evidence만으로 normative Decision을 만드는 proposal은 거부한다.

## 10. Event와 모델 비결정성

### 10.1 Event 종류

- PlanningSessionStarted
- RepoEvidenceRefreshed
- AnswerSubmitted
- AuditApplied
- SpecCandidateGenerated
- SpecApproved
- SpecRevisionRequested
- PlanCandidateGenerated
- PlanApproved
- PlanRevisionRequested
- LegacyContextImported

각 event payload에는 command의 주 효과와 effects 배열을 함께 넣는다. 질문 생성, EntitiesInvalidated, ApprovalsRevoked는 독립 event가 아니라 AuditApplied 또는 해당 command event의 effects 값이다.

### 10.2 Envelope

~~~rust
pub struct EventEnvelope {
    pub event_id: EventId,
    pub session_id: SessionId,
    pub seq: u64,
    pub revision_after: u64,
    pub domain_revision_after: u64,
    pub plan_revision_after: u64,
    pub schema_version: u32,
    pub event_type: EventType,
    pub semantic_payload: serde_json::Value,
    pub semantic_payload_hash: SemanticHash,
    pub metadata: EventMetadata,
    pub state_hash_after: StateHash,
}

pub struct EventMetadata {
    pub occurred_at: Timestamp,
    pub actor: Actor,
    pub adapter: AdapterKind,
    pub request_id: Option<RequestId>,
    pub command_id: CommandId,
}
~~~

metadata는 audit용이다. reducer와 adapter equivalence는 semantic payload만 사용한다.

seq와 revision_after는 1부터 시작하고 항상 같다. reducer는 이전 event의 revision_after + 1인지 확인한다. domain_revision_after와 plan_revision_after도 reducer가 command effect에서 계산한 값과 일치해야 한다. 이 세 값은 replay에 필요한 semantic field이며 metadata가 아니다.

aggregate semantic payload 공통 shape:

~~~json
{
  "operation": "planning.audit.apply",
  "primary": {"mode":"delta","proposal":{}},
  "effects": [
    {"kind":"entity_revised","entity_id":"REQ-001","revision":2},
    {"kind":"question_set","question_id":"qst_..."},
    {"kind":"artifact_invalidated","artifact":"spec","candidate_id":"cand_..."}
  ]
}
~~~

effect enum도 deny_unknown_fields를 사용한다. reducer는 primary와 effects를 다시 검산하며 저장된 revision_after를 맹신하지 않는다.

### 10.3 비결정성 처리

- 같은 input hash에서 모델이 다른 proposal을 내는 것은 정상이다.
- 적용된 proposal 원문은 event에 저장한다.
- 재평가는 새 work item과 command_id로 수행한다.
- 이전 assessment를 덮어쓰지 않는다.
- replay는 모델을 다시 호출하지 않는다.
- 모델 fingerprint는 기록 가능하지만 재현성 근거로 쓰지 않는다.
- 승인은 모델 이름이 아니라 exact candidate content에 결박한다.

## 11. Hash와 invalidation

### 11.1 Canonicalization

- UTF-8
- Unicode NFC
- line ending LF
- 모든 text field는 leading·trailing blank line을 제거한다.
- 각 line의 trailing space와 tab을 제거한다.
- prose의 내부 space, indentation, line break는 보존한다.
- 따라서 CRLF/LF와 trailing whitespace 차이는 hash에 영향이 없지만 내부 문장 공백 변화는 의미 변화다.
- JSON object key 사전식 정렬
- ordered sequence 순서 유지
- set 성격 배열은 stable ID 기준 정렬
- timestamp, actor, request_id, command_id 제외
- Markdown formatting 제외
- SHA-256, sha256:<lowercase hex> 형식

### 11.2 Hash

| Hash | 입력 |
| --- | --- |
| transcript_hash | initial request, ordered question, raw answer, selected choices |
| evidence_hash | repo identity, cited path와 file digest |
| audit_input_hash | transcript, evidence, current entity revisions, blockers, rubric version |
| spec_semantic_hash | canonical spec content와 entity revision refs |
| plan_input_hash | approved spec ID/hash, evidence hash, plan_revision, schema version |
| plan_semantic_hash | steps, dependencies, requirement refs, verifications, risks, rollback |
| normalized_state_hash | metadata를 제거하고 generated UUID를 canonical alias로 치환한 state |

generated ID canonical alias:

- 모든 generated record는 created_event_seq와 created_ordinal을 저장한다.
- created_ordinal은 aggregate event의 primary와 effects 배열에서 생성된 같은 kind record의 0-based 순서다.
- normalized form은 UUID bytes를 KIND@created_event_seq:created_ordinal로 치환한다.
- 예: QUESTION@5:0, BLOCKER@7:1, SPEC_CANDIDATE@12:0.
- session alias는 SESSION@1:0이다.
- human entity ID와 entity revision은 그대로 사용한다.
- state의 ID 본문과 모든 reference를 같은 alias map으로 치환하므로 관계가 보존된다.
- actor, timestamp, request_id, command_id, adapter, raw UUID는 normalized form에서 제외한다.

adapter가 같은 logical aggregate event와 effect 순서를 만들면 실제 UUID가 달라도 normalized_state_hash가 같다.

### 11.3 Invalidation

- answer 또는 spec-domain entity 변경: domain_revision 증가, full audit와 모든 artifact stale.
- evidence refresh에서 digest 변화: 영향 Fact와 depends_on downstream을 stale.
- 영향 범위를 계산할 수 없으면 모든 repo-derived Fact와 downstream을 stale.
- spec approval 변경: plan_revision 증가, plan candidate·approval stale.
- plan feedback: plan_revision 증가, spec approval 유지.
- Markdown 수정: canonical state 불변. status의 ObservedHealth.projection_status만 conflict.

## 12. Repo evidence

### 12.1 범위

v1 canonical evidence는 project-local file과 Git metadata만 지원한다.

DB에 source file 원문 전체를 복사하지 않는다. 다음만 저장한다.

- project-relative path
- line range
- size
- SHA-256
- tracked 또는 untracked 상태
- captured_at

Fact.statement에는 모델 요약이 들어갈 수 있다.

### 12.2 Git identity

~~~rust
pub struct GitRepoIdentity {
    pub head_oid: Option<String>,
    pub head_ref: Option<String>,
    pub dirty: bool,
    pub status_hash: String,
    pub cited_files_hash: String,
}
~~~

- status_hash는 git status --porcelain=v2 -z --untracked-files=all 출력의 hash다.
- ignored file과 .megara/는 제외한다.
- detached HEAD와 unborn repository를 허용한다.
- dirty worktree를 허용하되 정확한 status_hash에 결박한다.

non-Git project는 canonical project root와 cited_files_hash를 사용한다. 인용되지 않은 파일 변화는 자동 stale 원인이 아니다.

### 12.3 Path 보호

다음 경로는 evidence로 거부한다.

- project root 밖
- project 밖으로 나가는 symlink
- .megara/
- .git/
- Git ignored file
- .env
- pem, key, credential 계열 파일명

추적되는 .env.example 같은 명시적 예제 파일은 허용한다.

### 12.4 Freshness

status, current, spec show, plan show와 다음 sensitive operation에서 core가 현재 health를 다시 계산한다.

- full audit
- spec generate·approve
- plan generate·approve
- approved artifact bundle export

- HEAD OID
- status hash
- cited file digest와 존재 여부

query는 stale 경고와 evidence_refresh next action을 반환한다. sensitive mutation과 bundle export는 적용하지 않고 EVIDENCE_STALE을 반환한다.

evidence refresh는 모든 phase에서 허용한다.

- 새 snapshot이 저장된 snapshot과 같으면 event와 revision을 만들지 않고 현재 phase를 유지한다.
- 다르면 RepoEvidenceRefreshed aggregate event 하나를 append한다.
- 영향 Fact와 downstream entity를 stale 처리한다.
- domain_revision을 증가시키고 full audit와 모든 artifact approval을 revoke한다.
- 기존 pending_question과 required_model_action을 취소한다.
- phase를 Interview로 바꾸고 DeltaAudit work item을 만든다.

외부 웹 자료는 모델 참고자료일 뿐 자동 freshness source가 아니다. 핵심 외부 판단은 UserAnswer 또는 Assumption으로 기록한다.

## 13. 인터뷰와 모델 proposal

### 13.1 ModelWorkItem

PlanningCore는 모델을 호출하지 않고 다음 work item을 반환한다.

~~~rust
pub enum ModelActionKind {
    DeltaAudit,
    FullAudit,
    GenerateSpec,
    GeneratePlan,
}
~~~

~~~json
{
  "kind": "delta_audit",
  "work_item_id": "wrk_...",
  "session_id": "pln_...",
  "base_revision": 5,
  "base_domain_revision": 4,
  "base_plan_revision": 0,
  "input_hash": "sha256:...",
  "output_schema": "megara.audit-proposal/v1",
  "context": {
    "initial_request": "...",
    "latest_answer": {},
    "current_entities": [],
    "stale_entities": [],
    "current_edges": [],
    "blockers": [],
    "repo_snapshot": {},
    "question_authoring": {
      "version": "megara.question-authoring/v1",
      "rules": [
        {"id":"audience","instruction":"Megara와 구현 기술을 모르는 소프트웨어 기획 초심자를 독자로 둔다."},
        {"id":"context","instruction":"쉬운 말 2~4문장으로 배경과 지금 결정할 이유를 설명한다."},
        {"id":"one-decision","instruction":"한 번에 하나의 결정만 묻는다."},
        {"id":"terms","instruction":"전문용어를 피하고, 불가피하면 뜻뿐 아니라 이 문맥의 역할과 중요성을 설명한다."},
        {"id":"choices","instruction":"각 선택지의 진행 방향, 장점, 감수할 점을 서로 겹치지 않게 설명한다."},
        {"id":"impact","instruction":"답에 따라 spec 또는 plan의 무엇이 달라지는지 설명한다."},
        {"id":"recommendation","instruction":"유효한 근거를 연결할 수 있을 때만 추천한다."}
      ]
    }
  }
}
~~~

DeltaAudit context는 latest answer, current/stale entity revisions, current non-retired edges, blockers와 repo snapshot을 포함하고, FullAudit context는 ordered transcript 전체와 같은 graph/evidence context를 포함한다. `current_edges`는 stable edge ID 순서로 정렬하며 stale recovery가 필요한 entity와 validity causes를 숨기지 않는다. `input_hash`는 이 semantic context만 canonical hash한 값이고, `work_item_id`는 여기에 session, action kind, base revisions와 output schema/version을 별도로 결박한 session-bound identity다.

question_authoring은 next_question을 만들 수 있는 DeltaAudit와 FullAudit work item에 필수다. rules는 위 ID와 순서를 고정하고 빈 문자열을 허용하지 않는다. version과 rules 전체는 input_hash에 포함하므로 계약이 바뀌면 기존 proposal을 적용할 수 없다. 이 배열이 모델에 전달되는 정적 prompt contract이며 adapter가 별도 문구로 치환하지 않는다.

### 13.2 DeltaAudit

평상시에는 전체 transcript를 다시 분석하지 않는다.

~~~text
answer
  → latest answer + current graph
  → typed entity/edge/blocker operations
  → 다음 한 질문 또는 FullAudit 요청
~~~

DeltaAudit는 continue 또는 request_full_audit만 제안한다. 직접 ready를 선언할 수 없다.

QuestionProposal은 질문 내용과 답변 방식을 분리한다. 사용자가 Megara, Rust, MCP 또는 저장 방식에 대한 사전 지식이 없다는 전제로 작성한다.

선택형 질문의 exact shape:

~~~json
{
  "context": "예전에 Megara가 만든 작업 기록이 프로젝트에 남아 있을 수 있습니다. 이 기록은 새 기획 구조가 확인한 정보가 아니므로, 그대로 확정된 결정으로 취급하면 잘못된 계획이 이어질 수 있습니다.",
  "question": "예전 작업 기록을 새 기획에서 어떻게 다룰까요?",
  "why_it_matters": "이 선택에 따라 과거 기록을 참고자료로 보존할지, 현재 요청만으로 새 기획을 시작할지가 정해집니다.",
  "technical_terms": [],
  "source_refs": [{"kind":"entity","id":"DEC-002","revision":1}],
  "answer": {
    "mode": "choice",
    "choices": [
      {
        "id": "reference-only",
        "label": "참고자료로만 가져오기",
        "direction": "기록은 보존하되 새 명세의 확정 내용으로 사용하지 않고, 필요한 결정은 인터뷰에서 다시 확인합니다.",
        "benefits": ["과거 기록을 잃지 않고 필요한 부분을 다시 살펴볼 수 있습니다."],
        "tradeoffs": ["확인해야 할 질문이 늘어날 수 있습니다."]
      },
      {
        "id": "start-fresh",
        "label": "가져오지 않고 새로 시작하기",
        "direction": "과거 기록을 새 기획에 넣지 않고 현재 요청부터 인터뷰를 시작합니다.",
        "benefits": ["기획 상태가 단순하고 과거의 잘못된 판단이 섞이지 않습니다."],
        "tradeoffs": ["예전에 정한 내용도 필요하면 다시 답해야 합니다."]
      }
    ],
    "recommendation": {
      "choice_id": "reference-only",
      "reason": "기존 기록을 잃지 않으면서도 확인되지 않은 내용을 자동으로 승인하지 않는 방향입니다.",
      "source_refs": [{"kind":"entity","id":"DEC-002","revision":1}]
    },
    "freeform_hint": "두 선택지 중 하나를 고르거나, 원하는 처리 방식을 직접 설명해 주세요. 예: ‘결정 기록만 참고자료로 남기고 실행 기록은 제외해 주세요.’"
  }
}
~~~

자유 입력형 질문은 같은 공통 필드와 다음 answer variant를 사용한다.

~~~json
{
  "mode": "freeform",
  "freeform_hint": "사용자가 성공했다고 느낄 수 있는 결과를 설명해 주세요. 예: ‘새 프로젝트에서 10분 안에 검토 가능한 명세와 계획이 만들어진다.’"
}
~~~

필드와 variant 계약:

- context, question, why_it_matters, freeform_hint는 필수이며 trim 후 비어 있을 수 없다.
- technical_terms는 필수 배열이며 사용하지 않을 때는 빈 배열이다. 각 항목은 non-empty term과 plain_explanation을 가진다. NFC 정규화 후 trim한 term은 질문 안에서 중복될 수 없다.
- source_refs는 비어 있을 수 없고 기존 SourceRef 무결성 검사를 통과해야 한다.
- choice mode의 choices는 최소 2개다. 각 choice는 non-empty id, label, direction과 각각 최소 한 개의 non-empty benefits, tradeoffs를 가진다. id는 질문 안에서 unique다.
- choice mode의 recommendation은 null 또는 완전한 object다. object이면 choice_id는 같은 질문의 choice를 가리키고 reason과 source_refs는 비어 있을 수 없다. recommendation의 source_refs도 기존 무결성 검사를 통과한다.
- choice mode도 자유 입력을 항상 허용한다. freeform_hint는 번호나 label 외에 원하는 방향을 직접 답할 수 있음을 설명한다.
- freeform mode는 mode와 freeform_hint만 허용한다. choices와 recommendation을 넣으면 deny_unknown_fields와 tagged-union 검증으로 거부한다.
- 기존 text, recommended_answer, label-only choice는 v1 schema가 아니다. alias나 호환 역직렬화를 두지 않는다.

질문 작성 규칙은 `megara.question-authoring/v1`로 versioning한다. next_question을 만들 수 있는 DeltaAudit와 FullAudit work item은 다음 rubric을 정적 instruction으로 포함하고, Codex와 Pi는 바꾸지 않고 현재 host model에 전달한다. 모델은 QuestionProposal을 제출하기 전에 일곱 항목을 모두 확인한다.

1. 독자는 Megara, Rust, MCP, 데이터베이스 구조를 모르는 소프트웨어 기획 초심자다.
2. context는 쉬운 말 2~4문장으로 배경과 지금 결정해야 하는 이유를 설명한다.
3. question은 한 번에 하나의 결정만 묻는다.
4. 전문용어, 약어, 영문 식별자는 가능한 한 일상어로 바꾼다. 불가피하면 모두 technical_terms에 넣고, plain_explanation은 동의어나 약어 확장만 제시하지 않고 이 문맥에서 무엇을 하며 왜 중요한지 설명한다.
5. 각 choice의 label은 일상어로 쓴다. direction은 선택 뒤 상태나 다음 진행이 어떻게 달라지는지, benefits와 tradeoffs는 사용자가 얻는 점과 감수할 점을 설명하며 label이나 서로를 반복하지 않는다.
6. why_it_matters는 답에 따라 spec 또는 plan의 무엇이 달라지는지 설명한다.
7. recommendation은 source_refs로 근거를 연결할 수 있을 때만 만들고, 근거가 부족하면 null로 둔다.

rubric은 일곱 항목을 모두 만족해야 통과다. 예를 들어 `WAL을 사용합니까?`라는 질문에 `WAL = write-ahead log`만 덧붙이는 것은 실패다. `WAL = 프로그램이 갑자기 종료되어도 마지막 저장 내용을 복구할 수 있도록, 본 저장 전에 변경 기록을 따로 남기는 방식`처럼 사용 목적과 영향을 설명해야 한다. `opaque import`와 같이 용어만 있는 선택지나 `direction: opaque import한다`처럼 label을 반복하는 설명도 실패다.

검증 책임은 다음처럼 나눈다.

| 검사 | Rust core | 모델 authoring rubric |
| --- | --- | --- |
| 필수 필드, trim 후 빈 값, 배열 cardinality | 결정론적으로 거부 | 해당 없음 |
| answer mode별 허용 field | tagged union으로 거부 | 올바른 variant 선택 |
| choice ID unique, recommendation 참조 무결성 | 결정론적으로 거부 | 해당 없음 |
| technical term과 설명의 존재·중복 | 결정론적으로 거부 | 불가피한 용어를 빠짐없이 식별하고 쉽게 설명 |
| 초심자가 실제로 이해할 수 있는가 | 완전 판정하지 않음 | 일곱 항목 모두 자체 검토 |
| 질문이 한 결정만 묻는가 | 완전 판정하지 않음 | 한 문항 원칙 검토 |
| direction·benefit·tradeoff가 실질적이며 비반복인가 | 구조만 검사 | 의미와 차이를 검토 |
| 추천 이유가 근거 내용과 부합하는가 | SourceRef 존재와 유효성만 검사 | 근거 적합성 검토 |

`test/fixtures/planning/question_authoring/`에는 초심자용 gold fixture와 용어로 설명을 대체한 anti-example을 함께 둔다. prompt contract test는 work item에 rubric version과 일곱 규칙이 포함되고 두 adapter가 그대로 전달하는지를 검사한다. 고정 fixture는 release review에서 같은 일곱 항목을 yes/no로 확인하며 하나라도 no이면 통과하지 않는다. CI와 Rust가 자연어 이해도를 기계적으로 보증한다고 주장하지 않는다.

유효하지 않은 QuestionProposal은 PROPOSAL_SCHEMA_INVALID로 거부한다. event, revision, pending_question은 바뀌지 않고 기존 required_model_action이 남는다. host model은 같은 work_item_id, base revision, input hash와 새 command_id로 수정 proposal을 다시 제출할 수 있지만 core는 자동 retry하지 않는다. adapter는 누락 field 보완, 기본값 삽입, 추천 재선택, 질문 재작성 또는 별도 model 호출을 하지 않는다.

Planning Core 구현 전 확정하는 v1 계약이므로 기존 계획 문서의 text, recommended_answer, label-only choice를 원자적으로 교체한다. 신규 fixture와 호출부만 새 schema로 작성하고 별도 DB migration이나 compatibility shim을 만들지 않는다. legacy planning 자료는 opaque context로만 import하며 과거 QuestionProposal을 역직렬화하지 않는다.

### 13.3 FullAudit

spec 후보 직전에 현재 전체 입력으로 수행한다.

입력:

- initial request
- ordered transcript 전체
- current entity revisions
- blockers
- repo evidence snapshot
- audit rubric version

필수 출력:

~~~json
{
  "schema": "megara.audit-proposal/v1",
  "mode": "full",
  "work_item_id": "wrk_...",
  "base_revision": 12,
  "base_domain_revision": 8,
  "input_hash": "sha256:...",
  "readiness": "ready",
  "counterexample_review": {
    "performed": true,
    "challenged_entity_ids": ["REQ-002", "DEC-001"],
    "findings": []
  },
  "entity_ops": [],
  "edge_ops": [],
  "blocker_ops": [],
  "next_question": null
}
~~~

FullAudit가 entity를 바꾸면 Interview에 남고 domain_revision을 증가시킨다. 새 input hash로 FullAudit를 다시 수행해야 한다.

counterexample_review finding은 statement, result=resolved|blocking|advisory, non-empty source_refs를 가진다. result=blocking이면 대응 BlockerOp create가 반드시 함께 있어야 한다.

### 13.4 Readiness gate

Specification으로 이동하려면 모두 참이어야 한다.

- current Problem 존재
- current Outcome 존재
- current Requirement 존재
- current NonGoal 존재
- current DecisionBoundary 존재
- 모든 Requirement에 AcceptanceCriterion 존재
- blocking blocker 0
- pending question 없음
- evidence current
- current audit_input_hash에 대한 FullAudit 존재
- counterexample_review.performed = true
- FullAudit 이후 domain_revision 불변

모호성 점수, dimension floor, threshold, pass streak는 schema에도 넣지 않는다.

### 13.5 Wire schema 정책

모든 proposal과 citation struct는 serde deny_unknown_fields를 사용한다. 알 수 없는 field, enum, operation은 전체 command를 거부하며 부분 적용하지 않는다.

공통 제한:

- UTF-8 JSON object
- 전체 payload 최대 4 MiB
- 일반 text field 최대 64 KiB
- path 최대 4 KiB
- ID와 temp_ref 최대 128 byte
- 한 proposal의 operation 합계 최대 10,000개
- 빈 ID, 빈 statement, 중복 temp_ref 금지
- base revision과 input hash 필수

Evidence citation 입력:

~~~json
{
  "schema": "megara.evidence-citations/v1",
  "base_revision": 5,
  "citations": [
    {
      "temp_ref": "cite_cli",
      "path": "src/cli.rs",
      "ranges": [{"start_line": 1, "end_line": 220}],
      "claim": "CLI command enum은 src/cli.rs가 소유한다."
    }
  ]
}
~~~

- path는 project-relative UTF-8 path다.
- range는 1-based inclusive이며 start_line <= end_line이다.
- ranges가 비면 파일 전체 digest만 저장하고 line claim은 허용하지 않는다.
- core가 path 보호와 실제 file digest를 확인한 뒤 EVID-001 같은 canonical Evidence ID를 반환한다.

SourceRef는 다음 tagged union만 허용한다.

~~~json
{"kind":"initial_request","id":"request"}
{"kind":"answer","id":"ans_..."}
{"kind":"evidence","id":"EVID-001"}
{"kind":"entity","id":"DEC-001","revision":2}
{"kind":"approved_spec","id":"cand_...","semantic_hash":"sha256:..."}
~~~

AuditProposal 전체 shape:

~~~json
{
  "schema": "megara.audit-proposal/v1",
  "work_item_id": "wrk_...",
  "mode": "delta",
  "base_revision": 5,
  "base_domain_revision": 4,
  "input_hash": "sha256:...",
  "entity_ops": [],
  "edge_ops": [],
  "blocker_ops": [],
  "readiness": "request_full_audit",
  "next_question": null,
  "counterexample_review": null
}
~~~

EntityOp는 다음 variant만 허용한다.

~~~json
{
  "op": "create",
  "temp_ref": "tmp_req_1",
  "kind": "requirement",
  "body": {"statement":"상태는 project-local이어야 한다.","priority":"must"},
  "source_refs": [{"kind":"answer","id":"ans_..."}]
}
~~~

~~~json
{
  "op": "revise",
  "entity_id": "REQ-001",
  "base_entity_revision": 1,
  "body": {"statement":"상태는 프로젝트당 단일 DB에 저장한다.","priority":"must"},
  "source_refs": [{"kind":"answer","id":"ans_..."}]
}
~~~

~~~json
{
  "op": "reject",
  "entity_id": "REQ-001",
  "base_entity_revision": 2,
  "reason": "v1 비목표로 확정됨",
  "source_refs": [{"kind":"answer","id":"ans_..."}]
}
~~~

- create의 body는 9.1 Entity 표의 kind별 exact field를 사용한다.
- revise는 새 entity revision을 만들고 자동으로 supersedes edge를 생성한다.
- reject는 새 rejected revision을 만들며 hard delete하지 않는다.
- 모델은 canonical ID, current flag, phase를 body에 넣을 수 없다.

EdgeOp:

~~~json
{
  "op": "add",
  "kind": "has_acceptance_criterion",
  "from": {"temp_ref":"tmp_req_1"},
  "to": {"entity_id":"AC-001","revision":1},
  "source_refs": [{"kind":"answer","id":"ans_..."}]
}
~~~

~~~json
{
  "op": "retire",
  "edge_id": "edge_...",
  "base_edge_revision": 1,
  "reason": "requirement revision 교체"
}
~~~

- endpoint는 entity_id+revision 또는 같은 proposal의 temp_ref다.
- 허용 kind와 방향은 9.2 목록으로 고정한다.
- 같은 current edge 중복과 dangling endpoint를 거부한다.

BlockerOp:

~~~json
{
  "op": "create",
  "temp_ref": "tmp_blocker_1",
  "kind": "open_decision",
  "severity": "blocking",
  "statement": "migration 삭제 권한이 확정되지 않았다.",
  "source_refs": [{"kind":"entity","id":"DEC-002","revision":1}]
}
~~~

~~~json
{
  "op": "resolve",
  "blocker_id": "blk_...",
  "base_blocker_revision": 1,
  "resolution": "사용자 답변으로 명시됨",
  "source_refs": [{"kind":"answer","id":"ans_..."}]
}
~~~

Audit 결과는 항상 다음 action 하나를 만든다.

Delta:

- readiness=request_full_audit: next_question=null, core가 FullAudit work item 생성.
- readiness=continue: next_question 필수, core가 pending_question 설정.

Full:

- readiness=ready: entity_ops, edge_ops, blocker_ops가 모두 비어 있고 next_question=null. core가 Specification으로 전환.
- readiness=continue + operation 하나 이상: next_question=null. core가 operation 적용 뒤 새 input hash의 DeltaAudit work item 생성.
- readiness=continue + operation 0개: next_question 필수. core가 pending_question 설정.

그 밖의 조합은 PROPOSAL_SCHEMA_INVALID다. pending_question, required_model_action, approval_required, complete 중 현재 next action이 정확히 하나여야 한다. counterexample_review는 full에서 필수, delta에서 null이다.

## 14. Spec candidate와 승인

### 14.1 SpecProposal

SpecProposal은 current entity revision을 조립한다. 새 Requirement, Decision, AcceptanceCriterion을 만들거나 수정할 수 없다. 새 의미가 필요하면 Interview로 돌아간다.

~~~json
{
  "schema": "megara.spec-proposal/v1",
  "work_item_id": "wrk_...",
  "base_revision": 12,
  "base_domain_revision": 8,
  "audit_input_hash": "sha256:...",
  "title": "Planning Core v1",
  "summary": "Megara의 기획 상태와 전환 권한을 Rust core로 일원화한다.",
  "problem_ref": {"id":"PROB-001","revision":1},
  "outcome_refs": [{"id":"OUT-001","revision":1}],
  "decision_refs": [{"id":"DEC-001","revision":2}],
  "decision_boundary_refs": [{"id":"DBND-001","revision":1}],
  "requirement_refs": [{"id":"REQ-001","revision":2}],
  "acceptance_criterion_refs": [{"id":"AC-001","revision":1}],
  "constraint_refs": [{"id":"CON-001","revision":1}],
  "non_goal_refs": [{"id":"NG-001","revision":1}],
  "assumption_refs": [],
  "risk_refs": [{"id":"RISK-001","revision":1}],
  "advisories": []
}
~~~

- 모든 ref는 current entity revision이어야 한다.
- 같은 ref 중복을 거부한다.
- Requirement와 AcceptanceCriterion의 graph 연결을 다시 검증한다.
- advisories 항목은 statement String과 non-empty source_refs만 가진다. blocking blocker를 숨길 수 없다.
- CanonicalSpec은 위 exact field와 ref가 가리키는 current entity body로 구성한다.
- Markdown heading과 순서는 renderer version이 결정하며 proposal field가 아니다.

~~~rust
pub struct SpecCandidate {
    pub candidate_id: CandidateId,
    pub base_domain_revision: u64,
    pub audit_input_hash: SemanticHash,
    pub semantic_hash: SemanticHash,
    pub entity_refs: Vec<EntityRevisionRef>,
    pub content: CanonicalSpec,
    pub stale: bool,
}

pub struct ApprovalRef {
    pub candidate_id: CandidateId,
    pub semantic_hash: SemanticHash,
    pub base_revision: u64,
    pub approval_event_seq: u64,
}
~~~

approved_by, approved_at, adapter는 ApprovalRef나 normalized state에 넣지 않는다. 승인 event metadata에만 저장하고 history query가 event seq로 결합한다. reducer는 semantic payload와 approval_event_seq만으로 승인 상태를 재생한다. adapter equivalence는 metadata의 actor와 timestamp를 비교하지 않는다.

### 14.2 승인 검증

Spec approve payload:

~~~json
{
  "candidate_id": "cand_...",
  "semantic_hash": "sha256:...",
  "base_domain_revision": 8
}
~~~

core는 candidate current, stale 아님, semantic hash 재계산 일치, base revision 일치, current full audit 일치, blocker 0, evidence current를 모두 확인한다.

사용자 authority:

- 사용자가 직접 실행한 CLI approve command
- approval_mode=prompt가 강제된 Codex 전용 MCP approve tool
- Pi의 user slash command와 확인 UI가 직접 실행한 CLI approve command

모델 proposal이나 일반 assistant 문구는 승인 actor가 될 수 없다.

신뢰 경계:

- model-facing Pi RPC는 spec approve, plan approve, purge operation을 USER_ENTRYPOINT_REQUIRED로 거부한다.
- Pi registerTool에는 approve와 purge를 등록하지 않는다.
- Pi slash command handler는 확인 UI 뒤 pi.exec로 exact CLI command를 실행한다.
- Codex는 generic mutation tool을 제공하지 않고 approve·purge를 별도 MCP tool로 제공한다.
- installer가 해당 MCP tool의 approval_mode=prompt를 항상 projection한다.
- CLI 직접 실행과 Codex host confirmation은 로컬 사용자 신뢰 경계다.
- 악성 로컬 process, 사용자가 수동으로 완화한 Codex approval policy, 변조된 adapter를 방어하는 것은 v1 threat model 밖이다.
- core는 entrypoint가 부여한 approval actor를 event metadata에 기록하며 request payload의 actor field는 받지 않는다.

## 15. Plan candidate와 구조 validator

### 15.1 PlanProposal 최소 구조

~~~json
{
  "schema": "megara.plan-proposal/v1",
  "work_item_id": "wrk_...",
  "base_revision": 17,
  "base_plan_revision": 3,
  "plan_input_hash": "sha256:...",
  "spec": {
    "candidate_id": "cand_...",
    "semantic_hash": "sha256:..."
  },
  "baseline": {
    "commands": [
      "cargo fmt --check",
      "cargo clippy --all-targets -- -D warnings",
      "cargo test --all-targets"
    ],
    "known_failure_policy": "base commit에서 재현되는 동일 diagnostic만 baseline failure"
  },
  "steps": [
    {
      "temp_ref": "step_domain",
      "objective": "Planning domain type과 invariant를 구현한다.",
      "requirement_refs": [{"id":"REQ-001","revision":2}],
      "depends_on": [],
      "change_surface": ["src/planning/domain.rs","src/planning/engine.rs"],
      "risks": ["phase와 waiting 상태가 다시 혼합될 수 있다."],
      "rollback_or_recovery": "신규 planning module과 dispatch를 제거한다."
    }
  ],
  "verifications": [
    {
      "temp_ref": "verify_domain",
      "acceptance_criterion_ref": {"id":"AC-001","revision":1},
      "plan_step_refs": ["step_domain"],
      "method": "command",
      "procedure": "cargo test --test unit planning_domain",
      "expected_result": "모든 invariant test가 통과한다."
    }
  ],
  "plan_risks": [
    {
      "statement": "legacy migration 중단",
      "mitigation": "journal 기반 resume 또는 rollback"
    }
  ]
}
~~~

각 PlanStep:

- objective
- requirement_refs
- depends_on
- change_surface
- risks
- rollback_or_recovery

각 Verification:

- acceptance_criterion_ref
- plan_step_refs
- method: command, assertion, metric, manual
- procedure
- expected_result

Plan candidate는 approved spec candidate ID/hash, plan_input_hash, base_plan_revision을 가진다.

- baseline.commands는 하나 이상이어야 한다.
- step과 verification temp_ref는 각각 unique다.
- depends_on과 plan_step_refs는 같은 proposal의 step temp_ref만 가리킨다.
- unknown field를 거부한다.

### 15.2 Rust가 차단하는 것

- approved spec 없음 또는 ID/hash 불일치
- 존재하지 않거나 stale인 Requirement·AcceptanceCriterion 참조
- Requirement 참조가 없는 PlanStep
- PlanStep에 연결되지 않은 Requirement
- Verification이 없는 AcceptanceCriterion
- PlanStep에 연결되지 않은 Verification
- 존재하지 않는 dependency
- dependency cycle
- 빈 objective, change_surface, procedure, expected_result, rollback
- blocker 존재
- evidence stale
- plan input hash 또는 base_plan_revision 불일치

### 15.3 Rust가 보증하지 않는 것

- step이 requirement를 실제로 구현하는지
- 파일 범위가 충분한지
- verification이 실제 결함을 검출하는지
- rollback이 현실적인지
- 자연어 의미상 NonGoal을 침범하는지
- 보안·성능·운영 요구가 빠지지 않았는지

v1에서 planner는 current host model의 GeneratePlan 호출을 뜻한다. architect와 critic은 별도 agent, 상태, gate가 아니다. architecture와 contrarian 검토 항목은 PlanProposal rubric과 FullAudit rubric에 합친다.

### 15.4 Plan 승인 결박

~~~json
{
  "candidate_id": "cand_...",
  "semantic_hash": "sha256:...",
  "base_plan_revision": 3
}
~~~

core는 current approved spec ID/hash, plan_input_hash, candidate semantic hash, base_plan_revision, evidence health, structural blocker 0을 다시 확인한다. 한 항목이라도 달라지면 approval event를 만들지 않는다.

## 16. CLI 계약

### 16.1 Project argument

모든 command는 --project PATH를 받는다. 생략하면 cwd다.

### 16.2 Alias

~~~text
megara define "<request>"
  = megara planning start --request "<request>"

megara plan --session <id>
  = megara planning current --session <id>
~~~

plain shell의 megara plan은 LLM을 호출하지 않는다. approved spec이 있고 GeneratePlan이 필요하면 next_action.kind=generate_plan과 work item을 출력하고 성공으로 종료한다. Codex/Pi adapter는 그 work item을 current host model에 전달한 뒤 plan generate를 호출한다.

### 16.3 Session 선택

Mutation:

- start 외에는 --session 필수.
- active session fallback 없음.

Read-only:

1. 완료되지 않은 session이 정확히 하나면 선택.
2. 완료되지 않은 session이 없고 전체 session이 정확히 하나면 선택.
3. 그 외 SESSION_AMBIGUOUS.
4. session 0개면 SESSION_NOT_FOUND.

list는 session 선택을 하지 않는다.

### 16.4 Command tree

~~~text
megara planning start
  --request <text> [--title <text>]

megara planning answer
  --session <id> --question <id>
  (--text <answer> | --stdin)
  --expected-revision <n>

megara planning status
  [--session <id>]

megara planning current
  [--session <id>]

megara planning list
  [--phase interview|specification|planning|complete]

megara planning evidence refresh
  --session <id> --citations <path|->
  --expected-revision <n>

megara planning audit apply
  --session <id> --mode delta|full --proposal <path|->
  --expected-revision <n>

megara planning spec generate
  --session <id> --proposal <path|->
  --expected-revision <n> [--force]

megara planning spec show
  [--session <id>] [--candidate <id>] [--format markdown|json]

megara planning spec approve
  --session <id> --candidate <id> --semantic-hash <hash>
  --base-domain-revision <n> --expected-revision <n>

megara planning spec revise
  --session <id> --candidate <id>
  (--text <request> | --stdin)
  --expected-revision <n>

megara planning plan generate
  --session <id> --proposal <path|->
  --expected-revision <n> [--force]

megara planning plan show
  [--session <id>] [--candidate <id>] [--format markdown|json]

megara planning plan approve
  --session <id> --candidate <id> --semantic-hash <hash>
  --base-plan-revision <n> --expected-revision <n>

megara planning plan revise
  --session <id> --candidate <id>
  (--text <request> | --stdin)
  --expected-revision <n>

megara planning export
  [--session <id>] --out <path>
  [--format bundle|state-json|events-jsonl]
  [--include-transcript] [--force]

megara planning purge
  --session <id> --confirm <id> --expected-revision <n>

megara planning rpc
  --project <path>

megara planning mcp
  --project <path>

megara planning migrate
  --dry-run | --apply | --resume <migration-id> |
  --rollback <migration-id> [--force]
~~~

모든 명령은 --json을 지원한다. mutation의 --command-id가 생략되면 CLI가 UUIDv7을 만들고 응답에 포함한다.

### 16.5 Exit code

| Code | 의미 |
| --- | --- |
| 0 | 성공 |
| 2 | CLI 또는 schema 입력 오류 |
| 3 | revision, phase, stale, approval conflict |
| 5 | DB, corruption, filesystem 오류 |

start, answer, audit처럼 성공 뒤 model work item이 생기는 command도 exit 0이다. model action은 error가 아니라 result.next_action이다.

JSON 출력에서는 exit code와 별개로 typed error code를 반환한다.

## 17. 공통 request/response 계약

### 17.1 Logical request

CLI, MCP tool, Pi RPC는 내부에서 같은 envelope로 변환된다.

~~~json
{
  "protocol_version": 1,
  "request_id": "req_...",
  "operation": "planning.answer",
  "command_id": "cmd_...",
  "session_id": "pln_...",
  "expected_revision": 4,
  "params": {
    "question_id": "qst_...",
    "text": "planning TUI는 만들지 않는다.",
    "selected_choice_ids": []
  }
}
~~~

- request_id는 transport correlation 전용이다.
- command_id는 mutation idempotency key다.
- actor는 payload에서 받지 않고 entrypoint가 결정한다.
- Query는 command_id와 expected_revision이 없다.

canonical request hash 입력:

~~~text
protocol_version
project_id
operation
session_id 또는 null
expected_revision 또는 null
params의 canonical JSON
~~~

제외:

- request_id
- command_id
- actor와 adapter
- timestamp
- 원래 project path 문자열
- JSON formatting
- --json 출력 선택
- spec/plan generate의 ProjectionPolicy와 --force

project_id는 DB project_meta에 저장한 canonical project identity다. CLI, MCP, Pi가 같은 logical mutation과 command_id를 재전송하면 transport가 달라도 같은 request hash가 나와야 한다.

### 17.2 Success

~~~json
{
  "protocol_version": 1,
  "request_id": "req_...",
  "ok": true,
  "session_id": "pln_...",
  "revision": 5,
  "replayed": false,
  "result": {},
  "observed": {
    "projection_status": "unchanged",
    "evidence_current": true,
    "warnings": []
  }
}
~~~

result는 command_results에 저장하는 authoritative core result다. observed는 현재 filesystem·Git에서 매 호출마다 다시 계산하며 idempotency hash와 replay 대상이 아니다.

### 17.3 Error

~~~json
{
  "protocol_version": 1,
  "request_id": "req_...",
  "ok": false,
  "error": {
    "code": "REVISION_CONFLICT",
    "message": "expected revision 4 but current revision is 5",
    "retryable": false,
    "details": {
      "expected_revision": 4,
      "actual_revision": 5
    }
  }
}
~~~

### 17.4 Idempotency

- 동일 command_id + 동일 canonical request hash: 저장된 core result, replayed=true, 새 event 없음. observed health와 projection sync는 다시 계산 가능.
- 동일 command_id + 다른 hash: COMMAND_ID_REUSE.
- process restart 후에도 command_results로 유지한다.
- purge 뒤에는 purged_sessions의 purge receipt가 동일 purge command를 replay한다.
- purge로 지운 과거 command ID는 purged_command_ids에 ID만 남겨 COMMAND_ID_RETIRED로 거부한다.

### 17.5 Typed error

INVALID_REQUEST, PROTOCOL_VERSION_UNSUPPORTED, SESSION_REQUIRED, SESSION_AMBIGUOUS, SESSION_NOT_FOUND, SESSION_PURGED, REVISION_CONFLICT, COMMAND_ID_REUSE, COMMAND_ID_RETIRED, INVALID_PHASE, QUESTION_MISMATCH, PENDING_QUESTION_EXISTS, MODEL_ACTION_MISMATCH, PROPOSAL_SCHEMA_INVALID, PROPOSAL_BASE_MISMATCH, INVALID_SOURCE_REFERENCE, EVIDENCE_STALE, BLOCKERS_PRESENT, CANDIDATE_NOT_FOUND, CANDIDATE_STALE, APPROVAL_BINDING_MISMATCH, USER_ENTRYPOINT_REQUIRED, DB_BUSY, DB_CORRUPT, PROJECTION_DIVERGED, SCHEMA_UPGRADE_REQUIRED, SCHEMA_VERSION_UNSUPPORTED, IO_ERROR, PURGE_CONFIRMATION_MISMATCH, MIGRATION_INCOMPLETE, ROLLBACK_CONFLICT를 v1 enum으로 고정한다.

## 18. Adapter 계약

### 18.1 Pi one-shot JSON RPC

megara planning rpc는 stdin에서 UTF-8 JSON 한 줄을 읽고 stdout에 JSON 한 줄을 쓴 뒤 종료한다.

- 한 request, 한 response
- 최대 4 MiB
- pretty print 금지
- stdout에 protocol 외 출력 금지
- log는 stderr
- daemonize, retry queue, background process 없음

Pi extension은 각 tool 또는 slash command마다 child process를 spawn한다. command_id가 process restart와 중복 호출을 안전하게 만든다.

이 RPC entrypoint는 model-facing이다. planning.spec.approve, planning.plan.approve, planning.purge를 받으면 USER_ENTRYPOINT_REQUIRED를 반환한다.

### 18.2 Codex MCP stdio

Codex는 임의 JSONL command를 tool로 읽지 않는다. src/planning/mcp.rs는 MCP stdio server를 구현하고 같은 PlanningService를 직접 호출한다.

project .codex/config.toml projection:

~~~toml
[mcp_servers.megara_planning]
command = "/absolute/path/to/megara"
args = ["planning", "mcp", "--project", "/absolute/project/path"]
cwd = "/absolute/project/path"
enabled = true
startup_timeout_sec = 10
tool_timeout_sec = 120

[mcp_servers.megara_planning.tools.planning_spec_approve]
approval_mode = "prompt"

[mcp_servers.megara_planning.tools.planning_plan_approve]
approval_mode = "prompt"

[mcp_servers.megara_planning.tools.planning_purge]
approval_mode = "prompt"
~~~

project config는 trusted project에서만 로드된다. installer는 기존 config를 structured merge하고 다른 key를 보존한다. 충돌은 --force 없이 덮어쓰지 않는다.

merge 규칙:

- toml_edit로 mcp_servers.megara_planning table만 추가·갱신한다.
- 다른 top-level key, MCP server, comment, ordering은 보존한다.
- 기존 table이 Megara managed marker와 마지막 projected hash에 일치하면 갱신한다.
- 같은 이름의 unmanaged table이 있으면 기본 install은 conflict로 중단한다.
- --force는 해당 table만 backup 후 교체하며 config 전체를 재직렬화하지 않는다.
- uninstall은 current managed hash가 일치하는 table만 제거한다.

MCP tool:

- planning_start
- planning_answer
- planning_status
- planning_current
- planning_list
- planning_evidence_refresh
- planning_audit_apply
- planning_spec_generate
- planning_spec_show
- planning_spec_approve
- planning_spec_revise
- planning_plan_generate
- planning_plan_show
- planning_plan_approve
- planning_plan_revise
- planning_export
- planning_purge

query tool에는 readOnlyHint를, approve와 purge에는 host confirmation과 적절한 destructive annotation을 설정한다.

MCP initialize instructions의 첫 문장은 다음 의미를 담는다.

> Megara manages planning state only; use returned work items with the current host model, submit typed proposals, and never infer approval.

별도 Skill은 설치하지 않는다.

### 18.3 Pi extension

harness/pi/extensions/megara.ts를 다음 경계로 축소한다.

- model-facing planning tools를 pi.registerTool로 등록
- current work item을 현재 Pi model에 노출
- tool execute에서 one-shot RPC 호출
- model 또는 thinking level을 변경하지 않음
- role process, fallback model, retry loop, subagent tool 제거

사용자 authority가 필요한 동작은 pi.registerCommand로 제공한다.

- /megara-approve spec <session-id>
- /megara-approve plan <session-id>
- /megara-revise spec|plan <session-id>
- /megara-purge <session-id>

approve command는 current candidate ID, hash, base revision을 status로 읽고 Pi 확인 UI에 표시한다. 사용자가 확인한 뒤 pi.exec로 exact CLI approve command를 실행한다. purge도 같은 방식으로 direct CLI를 호출한다.

### 18.4 질문 표시 계약

검증된 PendingQuestion은 PlanningService의 pure `QuestionProjection` 함수에서 순서가 고정된 semantic block으로 변환한다. 이 projection은 DB나 이전 질문을 읽지 않고 입력만으로 같은 결과를 만들며, adapter가 질문 내용을 요약하거나 빠뜨리지 않게 하는 공통 표시 계약이다.

block 순서:

~~~text
technical_term*
  → context
  → question
  → why_it_matters
  → choice(label → direction → benefit* → tradeoff*)*
  → recommendation?
  → freeform_hint
~~~

- technical_terms가 비어 있지 않으면 `먼저 알아둘 말` 구역으로 가장 먼저 표시한다. 따라서 뒤에 나오는 불가피한 전문용어는 사용자가 뜻을 먼저 읽은 상태에서 접한다.
- technical_terms가 비어 있으면 technical_term block은 0개다.
- recommendation이 null이면 recommendation block은 0개다.
- freeform mode에는 choice와 recommendation block이 없다.
- 모든 사용자 표시 필드는 정확히 한 번 block에 투영하며 choice 순서, choice ID, recommendation의 target choice ID를 보존한다.
- QuestionProposal의 top-level source_refs는 projection root의 `question_source_refs` metadata에 정확히 한 번 복사한다.
- recommendation.source_refs는 recommendation block의 `source_refs` metadata에 정확히 한 번 복사한다.
- 두 source_refs metadata는 provenance와 진단용이며 기본 사용자 화면에는 표시하지 않는다. adapter는 이를 사람용 본문으로 중복 렌더링하지 않지만 structured result에서 삭제하지 않는다.

Codex MCP와 Pi extension은 같은 block sequence와 metadata를 전달한다. 색상, 줄바꿈, Markdown 장식과 viewport 배치는 달라도 되지만 block을 재작성, 병합, 생략하거나 순서를 바꿀 수 없다. adapter는 invalid QuestionProposal을 받지 않으며 누락된 설명을 자체 생성하지 않는다. 이 계약은 planning TUI를 추가하지 않는다.

contract test는 다음을 비교한다.

1. 적용 가능한 사용자 표시 field가 정확히 한 번 존재한다.
2. block kind와 choice 내부 순서가 위 계약과 같다.
3. choice ID와 recommendation target 관계가 보존된다.
4. question_source_refs와 recommendation source_refs가 지정된 metadata 위치에 한 번만 있고 서로 섞이거나 누락되지 않는다.
5. Codex와 Pi의 normalized semantic block sequence와 metadata가 같다.

### 18.5 Adapter equivalence

Codex와 Pi가 byte-for-byte 같은 event를 만든다고 주장하지 않는다.

다를 수 있는 값:

- event ID
- timestamp
- actor 표현
- request_id와 command_id
- generated candidate·question UUID

동등성 기준:

1. 같은 logical scripted input의 normalized_state_hash가 같다.
2. metadata를 제거하고 generated UUID를 KIND@created_event_seq:created_ordinal alias로 치환한 semantic event sequence가 같다.

## 19. Projection, export, purge

### 19.1 Markdown projection

spec.md와 plan.md는 generated header를 가진다.

~~~markdown
<!--
Generated by Megara Planning Core.
Do not edit directly.
session_id: pln_...
candidate_id: cand_...
semantic_hash: sha256:...
base_revision: 8
-->
~~~

write 순서:

~~~text
render to temp
  → write and fsync
  → existing digest 확인
  → atomic rename
~~~

기존 파일 digest가 마지막 manifest와 다르면 projection_status=conflict warning을 반환하고 파일을 쓰지 않는다. candidate 생성 DB mutation은 이미 성공한 상태다. --force는 post-commit projection overwrite에만 적용되며 DB command invariant를 우회하지 않는다.

명시적 doctor 또는 projection repair에서 conflict를 오류로 다룰 수 있지만, 이미 commit된 mutation을 실패나 rollback으로 표현하지 않는다.

spec/plan generate의 --force는 core request가 아니라 post-commit ProjectionPolicy다. command request hash에서 제외한다. 같은 command_id를 replay하면서 protect에서 overwrite로 바꿔도 core event는 재생성되지 않고 projection sync만 다시 수행한다.

### 19.2 Export

기본 bundle:

~~~text
manifest.json
spec.md
plan.md
~~~

raw answer, model proposal, event payload는 --include-transcript가 없으면 제외한다. 기존 output은 --force 없이 덮어쓰지 않는다. approved artifact bundle은 evidence current일 때만 export한다. state-json과 events-jsonl은 recovery 목적이라 stale 상태에서도 허용한다.

### 19.3 Purge

append-only는 purge되지 않은 session에 적용한다.

purge:

~~~text
BEGIN IMMEDIATE
  → confirmation, revision, command 검증
  → session과 연결된 migration backup ID 확인
  → 삭제할 session command_id를 purged_command_ids에 복사
  → session command_results 삭제
  → session events 삭제
  → session row 삭제
  → purge command_id, request hash, core result를 포함한 purged_sessions receipt 삽입
COMMIT
  → artifact directory 삭제
  → 연결된 migration backup directory 삭제
  → wal_checkpoint(TRUNCATE)
  → VACUUM
  → wal_checkpoint(TRUNCATE)
~~~

tombstone은 cleanup 완료 뒤 session_id, purged_at, purge_schema_version, purge_command_id, purge request hash, 최소 purge response, cleanup_state=clean만 남긴다. 제목, transcript, entity, evidence path, artifact hash는 남기지 않는다.

- 같은 purge command_id와 같은 request는 tombstone의 core result를 replay한다.
- 같은 purge command_id와 다른 request는 COMMAND_ID_REUSE다.
- 삭제된 과거 command_id 재사용은 COMMAND_ID_RETIRED다.
- purged_command_ids에는 random command_id와 session_id만 남기고 request·response 내용은 남기지 않는다.

LegacyContextImported session의 purge는 연결된 migration backup도 기본 삭제한다. privacy purge가 rollback 가능성보다 우선하며 별도 preserve option은 제공하지 않는다. v1 migration 하나는 planning session을 최대 하나만 만들므로 backup 공유 문제를 허용하지 않는다.

artifact 또는 backup 삭제, checkpoint, VACUUM이 commit 뒤 실패하면 session은 논리적으로 purge된 상태를 유지한다. purged_sessions에 cleanup_state=pending과 pending_backup_id를 임시 보존하고 경고를 반환한다. doctor --repair가 잔여 artifact와 backup, DB page 정리를 재시도한 뒤 pending_backup_id를 null로 만든다. 이 pending 기간에는 최소 tombstone 보증이 아직 완료되지 않았음을 status와 doctor가 표시한다.

PRAGMA secure_delete, DELETE, WAL truncate, VACUUM까지 검증한다. SSD wear-leveling과 외부 backup까지 포함한 forensic secure erase는 보증하지 않는다. 부분 redaction은 v1에 없다.

## 20. Migration과 legacy 제거

### 20.1 원칙

- 기존 .agents/state → .megara/state migration은 먼저 유지한다.
- legacy 의미를 deterministic code로 추측하지 않는다.
- legacy spec과 plan을 자동 승인하지 않는다.
- backup 성공 전 managed file을 삭제하지 않는다.
- 사용자 수정 파일은 --force 없이 삭제·덮어쓰기하지 않는다.
- clean project install은 planning workflow Skill과 lifecycle hook을 설치하지 않는다.

### 20.2 Migration command

~~~text
megara planning migrate --dry-run
megara planning migrate --apply
megara planning migrate --resume <migration-id>
megara planning migrate --rollback <migration-id> [--force]
~~~

apply:

1. legacy state, artifact, managed Skill·hook inventory 생성.
2. .megara/migration-backups/<id>/에 byte-for-byte backup과 manifest 저장.
3. legacy context가 있으면 Interview phase session 생성.
4. LegacyContextImported event에 source path, digest, raw artifact를 기록.
5. imported_legacy_context=true.
6. 다음 action을 DeltaAudit로 설정.
7. managed template와 hash가 같은 legacy projection만 removal candidate로 표시.
8. 사용자 수정 파일은 보존하고 warning.

raw legacy context는 새 entity가 아니다. host model의 audit와 사용자 승인을 다시 거친다.

migration-backups/<id>/manifest.json은 journal이기도 하다.

~~~text
prepared
  → planning_imported
  → projection_removed
  → applied
  → rolled_back
~~~

- 각 step 전후 manifest를 temp+fsync+atomic rename으로 갱신한다.
- apply가 중단되면 다음 apply는 MIGRATION_INCOMPLETE와 migration ID를 반환한다.
- resume은 manifest의 마지막 완료 step부터 idempotent하게 계속한다.
- rollback은 어느 intermediate state에서도 완료된 step을 역순으로 되돌린다.
- applied 또는 rolled_back은 terminal state다.

Planning DB 경계:

- prepared manifest에는 migration_id, project_id, source_bundle_hash를 저장한다.
- import command_id는 sha256(project_id, migration_id, "legacy-import", source_bundle_hash)에서 결정론적으로 만든 cmd_mig_* 값이다.
- planning_imported step은 SQLite transaction 하나에서 새 session과 LegacyContextImported aggregate event 하나를 만든다.
- 이 event가 새 session의 revision=1이며 initial request, raw legacy context, migration_id, source_backup_id, imported_legacy_context=true, DeltaAudit work item을 함께 담는다. PlanningSessionStarted event를 추가로 만들지 않는다.
- transaction core result의 session_id와 revision을 manifest에 기록한 뒤 journal을 planning_imported로 전환한다.
- DB commit 뒤 manifest write 전에 crash가 나면 resume이 같은 derived command_id를 재전송해 저장된 core result를 replay하고 journal만 복구한다.
- projection_removed 이후 filesystem step도 manifest의 source digest와 destination digest로 idempotency를 확인한다.
- rollback이 migration-created session을 purge할 때는 sha256(project_id, migration_id, "rollback-purge")로 파생한 별도 command_id를 사용한다.

rollback:

- manifest hash 검증
- 제거된 managed projection과 legacy state 복원
- migration-created session이 이후 변경되지 않았으면 purge
- 이후 변경됐다면 ROLLBACK_CONFLICT
- --force이면 session을 먼저 export한 뒤 purge

### 20.3 최종 삭제 범위

Planning Core와 adapter가 수용 기준을 통과한 뒤 삭제한다.

- Team, Ultragoal CLI와 runtime
- deep-interview, ralplan, team, ultragoal workflow Skill과 fragment
- UserPromptSubmit planning transition
- SessionStart를 포함한 managed Codex hooks.json projection 전체
- PreToolUse, PostToolUse, Stop, SubagentStart, SubagentStop
- git_guard와 mutation guard
- hidden workflow metadata parser
- execution retry와 continuation
- commit, subagent, goal receipt
- 관련 state, fixture, test

남는 hook은 없다. utility 기능이 남더라도 hook으로 자동 활성화하지 않는다.

비-workflow utility asset을 제거하려면 별도 제품 결정이 필요하다. 이 계획에서는 planning projector와 상태 엔진에 연결하지 않는다.

## 21. 실제 파일 경계

### 21.1 신규

~~~text
src/cli/planning.rs

src/planning.rs
src/planning/domain.rs
src/planning/engine.rs
src/planning/store.rs
src/planning/protocol.rs
src/planning/evidence.rs
src/planning/artifacts.rs
src/planning/mcp.rs
src/planning/migration.rs

src/targets/codex/planning.rs

test/fixtures/planning/
  protocol/
  question_authoring/
  replay/
  evidence/
  artifacts/
  legacy/
~~~

책임:

| File | 책임 |
| --- | --- |
| domain.rs | ID, phase, state, entity, edge, blocker, candidate, event type |
| engine.rs | command, query, invariant, transition, invalidation, validator |
| store.rs | SQLite schema, transaction, idempotency, replay, purge |
| protocol.rs | logical envelope, typed response/error, one-shot RPC, pure QuestionProjection |
| evidence.rs | project root, Git/non-Git identity, path 보호, freshness |
| artifacts.rs | proposal 검증, canonical hash, Markdown renderer, atomic write |
| mcp.rs | MCP initialize, tools/list, tools/call, annotation, PlanningService bridge |
| migration.rs | legacy inventory, backup manifest, opaque import, rollback |
| codex/planning.rs | project config MCP stanza와 merge 입력 |

### 21.2 수정

- Cargo.toml, Cargo.lock
- src/cli.rs
- src/main.rs
- src/doctor.rs
- src/installer/planner.rs
- src/installer/migration.rs
- src/installer/model.rs
- src/targets/codex.rs
- src/targets/codex/projection.rs
- src/targets/codex/config.rs
- src/targets/pi.rs
- src/templates/specs.rs
- harness/.gitignore
- harness/rules/planning.md
- harness/pi/extensions/megara.ts
- test/unit/main.rs
- test/integration/main.rs
- docs/index.md

### 21.3 Test

~~~text
test/unit/planning_domain.rs
test/unit/planning_engine.rs
test/unit/planning_store.rs
test/unit/planning_protocol.rs
test/unit/planning_artifacts.rs

test/integration/planning_cli.rs
test/integration/planning_mcp.rs
test/integration/planning_pi.rs
test/integration/planning_install.rs
test/integration/planning_migration.rs
test/integration/planning_purge.rs
test/integration/planning_adapter_equivalence.rs
~~~

현재 Cargo.toml은 autotests=false지만 이미 unit=test/unit/main.rs, integration=test/integration/main.rs 두 명시적 test target이 있다. 신규 [[test]] target은 만들지 않고 두 main.rs에 module을 명시적으로 등록한다.

### 21.4 책임 제한

- src/templates.rs에는 planning logic을 넣지 않는다.
- src/targets/codex.rs 밖에서 Codex config를 직접 쓰지 않는다.
- Pi extension은 .megara/planning을 직접 읽지 않는다.
- adapter에서 rusqlite를 import하지 않는다.
- ratatui module에서 planning을 import하지 않는다.

## 22. 구현 slice

각 slice는 하나의 독립 commit 의도를 가진다.

### Slice 0 — Baseline과 inventory

Dependency: 없음

변경:

- 현재 build/test 결과 기록
- tracked legacy path fixture 생성
- 기존 install, hook, Pi projection snapshot fixture 생성

Acceptance:

- base commit SHA, Cargo.lock hash, rustc -Vv, command exit code 기록
- legacy managed path가 재실행 시 동일
- 런타임 동작 변경 없음

Verification:

~~~bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --quiet -- docs check --root docs
~~~

Rollback: fixture와 baseline helper만 제거.

### Slice 1 — Domain과 in-memory engine

Dependency: Slice 0

변경:

- src/planning/domain.rs
- src/planning/engine.rs
- unit tests

Acceptance:

- phase에 waiting, approved, stale가 없음
- one-question invariant
- revision conflict
- source·edge 방향
- domain/plan invalidation
- exact approval binding type
- 모델과 filesystem 없이 transition test 가능

Verification:

~~~bash
cargo test --test unit planning_domain
cargo test --test unit planning_engine
~~~

Rollback: planning module 선언과 신규 파일 제거. 저장 상태 없음.

### Slice 2 — SQLite, replay, logical protocol

Dependency: Slice 1

변경:

- Cargo dependency
- store.rs, protocol.rs
- start, answer, status, current, list, purge
- one-shot RPC

Acceptance:

- WAL schema v1
- event와 state atomic commit
- 동일 command 재실행 idempotent
- 동일 command ID 다른 payload 거부
- mutation당 aggregate event 1개, seq=revision
- concurrent writer lost update 0
- replay 외부 I/O 0
- process restart 뒤 idempotency 유지
- purge 재시도는 receipt replay, 과거 command ID는 retired
- RPC stdout 한 JSON line

Verification:

~~~bash
cargo test --test unit planning_store
cargo test --test unit planning_protocol
cargo test --test integration planning_cli
~~~

Rollback: 사용자 공개 전 개발 fixture DB만 제거.

### Slice 3 — Evidence와 interview loop

Dependency: Slice 2

변경:

- evidence.rs
- AuditProposal과 work item
- tagged-union QuestionProposal과 pure QuestionProjection
- `megara.question-authoring/v1` 정적 rubric과 gold·anti fixture
- delta/full audit
- readiness gate

Acceptance:

- clean, dirty, detached, unborn, non-Git fixture
- HEAD, status, cited file 변화 감지
- ObservedHealth 변화가 event와 normalized state hash를 바꾸지 않음
- 모든 phase에서 evidence refresh 가능하고 변화 시 Interview 복귀
- root 밖 symlink와 secret path 거부
- Fact는 evidence source 필수
- Decision은 user source 필수
- delta가 직접 ready 전환 불가
- current full audit와 counterexample review 필수
- choice/freeform variant, 필수 field, choice cardinality, recommendation·SourceRef 무결성을 결정론적으로 검증
- invalid QuestionProposal은 event·revision·pending_question 변화 없이 거부하고 required_model_action 유지
- 모든 next_question 가능 work item이 같은 authoring rubric version과 일곱 규칙을 포함
- QuestionProjection이 모든 표시 field와 provenance metadata를 계약된 위치와 순서로 정확히 한 번 투영
- gold fixture는 초심자 rubric 일곱 항목을 모두 통과하고 anti-example은 실패 이유가 각 항목에 연결됨

Verification:

~~~bash
cargo test --test integration planning_cli
cargo test --test unit planning_engine
cargo test --test unit planning_protocol
~~~

Rollback: evidence·audit operation 제거. fixture DB schema는 개발 중 재생성.

### Slice 4 — Spec과 plan artifact

Dependency: Slice 3

변경:

- artifacts.rs
- spec/plan command
- semantic hash
- renderer와 projection conflict
- plan structural validator
- export

Acceptance:

- full audit 없이 spec 생성 불가
- spec/plan exact approval binding
- CRLF/LF와 trailing whitespace 차이는 semantic hash에 영향 없음
- 의미 변화는 hash 변경
- Markdown 삭제 후 재생성
- 직접 편집 기본 overwrite 차단
- projection conflict·I/O 실패가 committed candidate를 실패로 표현하지 않음
- idempotent replay가 core result를 유지하고 projection sync만 재시도
- 모든 Requirement와 AC 추적성 검사
- dependency cycle 차단
- plan revise는 spec approval 유지

Verification:

~~~bash
cargo test --test unit planning_artifacts
cargo test --test integration planning_cli
~~~

Rollback: artifact operation 제거, 개발 session export 후 purge.

### Slice 5 — Codex MCP와 Pi adapter

Dependency: Slice 4

변경:

- mcp.rs
- Codex project config structured merge
- Pi extension을 planning-only adapter로 rewrite하고 role/subagent/fallback/retry 코드를 이 slice에서 제거
- planning rule 축소
- runtime .gitignore

Acceptance:

- Codex MCP initialize, tools/list, tools/call conformance
- project .codex/config.toml에 absolute command·project root
- approve/purge tool host prompt
- Pi는 current model과 thinking level을 바꾸지 않음
- Pi role/subagent/retry code 없음
- adapter DB 직접 접근 없음
- normalized state와 semantic event equivalence
- Codex와 Pi가 같은 QuestionProjection block·metadata를 누락이나 재작성 없이 전달
- 전문용어가 있으면 `먼저 알아둘 말`을 질문 본문보다 먼저 표시
- 각 선택지의 label, direction, benefits, tradeoffs와 자유 입력 안내를 모두 표시
- planning TUI 없음
- 신규 planning adapter가 hook을 등록하지 않음. 기존 legacy hook 제거는 Slice 6

Verification:

~~~bash
cargo test --test integration planning_mcp
cargo test --test integration planning_pi
cargo test --test integration planning_install
cargo test --test integration planning_adapter_equivalence
cargo test --all-targets
~~~

Rollback: 신규 planning projection 제거. core CLI는 유지 가능.

### Slice 6 — Migration과 legacy runtime 제거

Dependency: Slice 5

변경:

- migration dry-run, apply, rollback
- crash-resumable backup journal
- opaque legacy import
- workflow Skill, hook, Team, Ultragoal Rust runtime 삭제
- 관련 CLI, state, tests 삭제

Acceptance:

- dry-run write 0
- backup 실패 시 removal 0
- 각 journal step 중단 뒤 resume 또는 rollback 가능
- DB commit 뒤 journal write crash를 같은 derived command_id로 복구하며 session 중복 0
- legacy artifact 자동 승인 0
- imported session은 Interview
- user-modified file 보존
- imported session purge가 linked migration backup도 제거
- rollback byte-equivalent 복원
- clean project install에 planning workflow Skill·hook 없음
- Codex hooks.json projection과 hook runtime 없음
- daemon, queue, auth, worktree, dashboard 추가 없음

Verification:

~~~bash
cargo test --test integration planning_migration
cargo test --test integration planning_install
cargo test --all-targets
rg -n 'deep-interview|ralplan|ultragoal|SubagentStart|SubagentStop|PreToolUse|PostToolUse' Cargo.toml Cargo.lock harness src test docs
~~~

허용 검색 결과는 src/planning/migration.rs, test/fixtures/planning/legacy/, docs의 비교·migration·history 설명뿐이다. 그 밖의 src, harness, Cargo dependency와 installer runtime projection의 허용 결과는 0개다.

Rollback: planning migrate --rollback 후 이전 Megara release 재설치.

### Slice 7 — Doctor와 release hardening

Dependency: Slice 6

변경:

- doctor integrity, replay, projection, tombstone, residue 검사
- crash, corruption, purge residue fixture
- docs와 release migration 안내

Acceptance:

- doctor 기본 실행은 read-only
- doctor --repair가 event를 수정하지 않음
- projection cache와 Markdown만 재생성
- purge residue 정리 재시도
- 모든 최종 수용 기준 통과

Verification:

~~~bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --quiet -- docs check --root docs
git diff --check
~~~

Rollback: doctor planning 진단만 제거. core data format은 유지.

## 23. Test matrix

| 영역 | 필수 case |
| --- | --- |
| Session | 0개, 1개 open, 여러 open, complete 혼합 |
| Question | pending 없음, 일치, mismatch, 중복 질문, choice/freeform variant |
| Question schema | 공통 field·공백 누락, choice 0·1개, blank·duplicate ID, direction·benefits·tradeoffs 누락·빈 원소, recommendation partial·unknown choice·invalid source, freeform 금지 field, blank·duplicate term, 빈 plain_explanation |
| Question invalidation | invalid proposal의 event·revision·pending question 변화 0, required model action 유지, corrected proposal 재제출 |
| Question authoring | rubric v1과 일곱 규칙 prompt contract, novice-readable gold fixture, jargon-only·label-repeat anti-example |
| Question projection | field당 block 1개, 순서, optional block 0개, choice·recommendation 관계, question·recommendation SourceRef metadata 분리 |
| Revision | current, stale, concurrent writer |
| Idempotency | 동일 ID/동일 payload, 동일 ID/다른 payload, restart |
| Phase | 모든 legal·illegal transition |
| Orthogonal state | waiting, blocker, stale, approval 조합 |
| Entity | create, revise, supersede, reject, invalid source |
| Edge | direction, missing ref, cycle |
| Audit | delta, full, hash mismatch, full 뒤 domain 변화 |
| Readiness | Problem, Outcome, Requirement, NonGoal, Boundary, AC 누락 |
| Model error | invalid JSON, unknown enum, missing field, oversized proposal |
| Evidence | clean, dirty, detached, unborn, non-Git |
| Evidence stale | HEAD, status, cited file 변경·삭제 |
| Evidence security | ignored file, env, symlink escape, .megara |
| Hash | NFC, LF/CRLF, object order, set order, semantic change |
| Spec | candidate, approval binding, revise, direct edit |
| Plan | orphan requirement, missing verification, stale ref, cycle |
| Replay | state cache 삭제, pure fold, hash mismatch |
| Event boundary | mutation당 event 1개, seq=revision, effects replay |
| Crash | transaction 전·중·후, projection temp file |
| Observed health | 외부 변화가 event 없이 canonical hash를 바꾸지 않음 |
| RPC | invalid JSON, line size, stdout 오염 |
| Approval entrypoint | model RPC approve·purge 거부, CLI·Codex prompt·Pi command 허용 |
| MCP | initialize, list, call, typed error, approval policy |
| Adapter | normalized state, semantic event equivalence, Codex·Pi QuestionProjection block·metadata 동일성 |
| Migration | dry-run, backup, interrupted apply, resume, modified file, rollback |
| Purge | rows, command retirement, purge replay, linked migration backup, pending cleanup, tombstone, WAL, VACUUM residue |
| Gitignore | DB, WAL, artifact, backup |
| Scope | planning TUI, hook, daemon, queue, auth, worktree 없음 |

## 24. Baseline 실패 구분

Slice 0에서 base commit 기준으로 다음을 기록한다.

- git commit SHA
- Cargo.lock SHA-256
- rustc -Vv와 cargo -V
- command와 exit code
- 실패 test 이름
- rustc 또는 clippy error code
- 경로·line number를 제거한 diagnostic 핵심 문구

Baseline failure:

- 같은 base commit, toolchain, Cargo.lock에서 재현
- 같은 test 또는 같은 error code와 핵심 문구
- planning patch가 없는 checkout에서도 발생

New failure:

- baseline에서 통과한 command 실패
- 신규 test 실패
- 기존 실패 수 증가
- 새 rustc·clippy diagnostic
- adapter, migration, fixture mismatch

각 slice는 unrelated baseline failure를 고칠 의무는 없다. 신규 failure는 추가할 수 없다. planning 관련 test는 baseline allow-list로 면제하지 않는다.

## 25. 최종 수용 기준

### Product

- Megara planning은 기획까지만 수행한다.
- 구현, commit, team, goal workflow를 시작하지 않는다.
- Skill과 hook이 planning state를 소유하지 않는다.

### State

- Rust core만 authoritative state를 바꾼다.
- 모델은 typed proposal만 제출한다.
- adapter는 DB와 projection을 직접 쓰지 않는다.
- 성공 mutation 하나가 aggregate event 하나와 revision 하나를 만든다.
- ObservedHealth는 canonical state와 replay hash 밖에 있다.
- 모든 transition과 invalidation이 test된다.

### Storage

- project-local 단일 SQLite DB.
- WAL transaction에서 event와 state cache atomic 갱신.
- replay 외부 I/O 0.
- projection 삭제 후 재생성 가능.

### Session과 protocol

- mutation은 명시적 session.
- read-only fallback은 유일한 경우만.
- command idempotency와 expected revision 강제.
- purge 뒤에도 purge command replay와 과거 command ID retirement 유지.
- imported session purge가 연결된 migration backup까지 제거하고 cleanup residue를 보고.
- RPC와 MCP가 typed error를 유지.

### Interview

- 질문 동시 1개.
- 모든 질문은 non-empty context, question, why_it_matters, freeform_hint를 가진다.
- 선택형 질문의 모든 선택지는 선택 뒤 방향, 장점, 감수할 점을 각각 설명한다.
- 불가피한 전문용어는 질문보다 먼저 plain_explanation을 표시하며 용어 또는 약어 풀이만으로 설명을 대체하지 않는다.
- 추천이 있으면 같은 질문의 선택지, 이유, 유효한 SourceRef를 proposal에 함께 포함하고 근거가 부족하면 추천하지 않는다.
- Codex와 Pi는 같은 QuestionProjection 의미 block과 provenance metadata를 누락·재작성 없이 전달한다.
- novice-readable 고정 fixture는 `megara.question-authoring/v1` 일곱 항목의 release review를 모두 통과한다.
- raw answer 보존.
- repo fact와 user decision source 분리.
- delta audit와 final full audit 분리.
- 숫자 threshold와 streak 없음.

### Approval

- candidate ID, semantic hash, base revision 모두 일치.
- stale candidate 승인 불가.
- model-facing Pi RPC의 approve·purge 거부.
- 사용자-confirmed entrypoint만 approval event 생성.

### Evidence

- HEAD, dirty status, cited file digest snapshot.
- dirty worktree 허용, exact snapshot 결박.
- repo 변화 stale 감지.
- secret·ignored path 거부.

### Artifact

- Markdown은 generated projection.
- 직접 편집은 canonical state에 반영되지 않음.
- 기본 write는 수정 파일 보호.
- export는 transcript 기본 제외.

### Migration

- backup 전 removal 0.
- legacy artifact 자동 승인 0.
- interrupted migration을 journal로 resume 또는 rollback.
- rollback 가능.
- 최종 planning projection에 lifecycle hook 없음.

### Scope

planning dependency 또는 runtime에 daemon, queue, polling, auth server, issue broker, worktree manager, dashboard, planning TUI, team, ultragoal, mutation guard를 추가하지 않는다.

## 26. 보증 문구

사용자 문서와 CLI는 “spec과 plan의 품질을 기계적으로 보증한다”라고 표현하지 않는다.

사용할 문구:

> Megara는 기획 상태, 요구사항과 계획의 추적성, artifact 최신성, revision invalidation, 구조적 완결성, 명시적 승인 이력을 기계적으로 검증한다. 의미적 정확성과 기술적 타당성은 현재 host model의 검토와 사용자 승인 대상이다.

## 27. 개발을 막지 않는 후속 개선

- Markdown 시각 스타일
- 추가 export 형식
- host model metadata 확장
- 외부 웹 evidence freshness adapter
- encrypted content store와 부분 redaction
- 수치 ambiguity 실험
- multi-project planning

planning TUI는 저장소 규칙이 바뀌기 전까지 후속 후보에도 넣지 않는다.

## 28. 참고 자료

- [grill-me / grilling](https://github.com/mattpocock/skills/blob/main/skills/productivity/grilling/SKILL.md)
- [Ouroboros interview](https://github.com/Q00/ouroboros/blob/main/skills/interview/SKILL.md)
- [Ouroboros ambiguity implementation](https://github.com/Q00/ouroboros/blob/main/src/ouroboros/bigbang/ambiguity.py)
- [OMC deep-interview](https://github.com/Yeachan-Heo/oh-my-codex/blob/main/skills/deep-interview/SKILL.md)
- [Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp.md)
- [Workflow UX audit](../references/workflow-ux-audit.md)

## 29. v1 보충 계약 — TO_DEFINE 해소

다음 결정은 저장소 현실과 v1 최소 범위에 맞춘 보충 계약이다. 이 절의 값은 구현·fixture·golden에서 고정하며, v1 범위를 넓히지 않는다.

| ID | 확정 계약 |
| --- | --- |
| `MPC-TDF-001` | `project_id`는 canonical project root의 UTF-8 절대 경로에 `NFC`와 `LF` 규칙을 적용한 뒤 `sha256`을 계산하고 `prj_<64 hex>`로 만든다. DB 최초 생성 시 저장하며, 기존 DB의 값과 현재 root가 다르면 `INVALID_REQUEST`와 `details.reason=project_id_mismatch`를 반환한다. relocation 지원은 v1에 없다. |
| `MPC-TDF-002` | 모든 logical operation은 `result.schema="megara.result/v1"`을 사용하며, request 필드·금지 필드·success result 필드·필수 여부는 `test/fixtures/planning/protocol/operations-v1.json`에 operation별로 열거한다. fixture의 17개 operation(`planning.start`, `answer`, `status`, `current`, `list`, `evidence.refresh`, `audit.apply`, `spec.generate`, `spec.show`, `spec.approve`, `spec.revise`, `plan.generate`, `plan.show`, `plan.approve`, `plan.revise`, `export`, `purge`)이 canonical golden 집합이다. aliases는 logical operation을 추가하지 않는다. 필드 누락·추가·타입 변경은 protocol golden 실패다. |
| `MPC-TDF-003` | managed planning projection의 설치 entrypoint는 `megara install --scope project --target <codex|pi>`이고 제거 entrypoint는 기존 `megara uninstall --scope project --target <codex|pi> [--force]`다. planning DB와 export는 uninstall 대상이 아니며, Codex managed MCP table·Pi managed extension만 해당 target projector가 보호 규칙에 따라 제거한다. `--force`는 충돌 파일의 backup 후 managed 대상만 교체·제거한다. |
| `MPC-TDF-004` | v1 host matrix는 Codex CLI `>=0.144.0`을 macOS arm64·Linux x86_64에서, Codex App `>=26.707.30751`을 macOS arm64에서, Pi `>=0.80.10` 및 `<0.81.0`을 macOS arm64·Linux x86_64에서 검증한다. Codex MCP stdio와 Pi extension API(`registerTool`, `registerCommand`, `exec`)를 해당 조합에서 사용한다. Windows와 global planning DB는 지원 claim에서 제외한다. 실제 host 확인은 automated protocol fixture와 별도의 manual gate로 판정한다. |
| `MPC-TDF-005` | evidence path는 대소문자를 접어 검사한다. basename이 `.env`이거나 basename에 `secret`, `credential`, `password`, `passwd`, `token`, `api_key`, `apikey` 중 하나가 포함되거나 확장자가 `pem`, `key`, `p12`, `pfx`, `der`면 거부한다. 단, tracked basename `.env.example`만 허용한다. `.env.sample`과 `.env.template`은 v1에서 허용하지 않는다. |
| `MPC-TDF-006` | Pi one-shot child timeout은 120초다. timeout 또는 abort 시 SIGTERM을 보내고 2초를 기다린 뒤 아직 살아 있으면 SIGKILL을 보내며, child 종료를 확인한 뒤 `IO_ERROR` typed response를 반환한다. retry·fallback·background child는 없다. |
| `MPC-TDF-007` | 표준 성능 fixture는 200 events, 100 citations, 50 current entities, 20 plan steps다. release profile(`cargo build --release`)로 fresh temporary project를 process-isolated 실행하고, 각 operation마다 warm-up 5회 후 timed sample 30회를 수집한다. clock은 Rust `std::time::Instant` monotonic clock, P95는 nearest-rank `ceil(0.95 * 30)` sample이다. macOS arm64 기준 start/status/replay/evidence refresh/migration/purge의 각 P95는 각각 500ms/500ms/1s/2s/2s/2s 이하여야 하며, writer lock 대기는 5초 계약을 따른다. Linux x86_64는 같은 절차로 별도 결과를 기록한다. 별도 runtime service나 지속 benchmark daemon은 추가하지 않는다. |
| `MPC-TDF-008` | JSON parser resource limit은 payload pre-scan nesting depth 64다. 65단계 이상은 `INVALID_REQUEST`로 parse·apply 전에 거부하고 event·revision은 변하지 않는다. 4MiB payload와 기존 field·operation limit을 함께 적용한다. |
| `MPC-TDF-009` | `target/megara-evidence/<release-commit>/`은 staging 전용이며 authoritative archive가 아니다. release CI가 `actions/upload-artifact@v4`로 immutable artifact `megara-planning-release-evidence-<release-commit>`를 업로드하고 `retention-days: 90`을 명시한다. archive에는 `manifest.json`, command stdout/stderr 원문, test report, DB/FS/protocol/migration/purge trace와 각 파일의 SHA-256이 포함되며, manifest는 release commit, Cargo.lock hash, toolchain, platform과 전체 파일 hash를 검증한다. `if-no-files-found: error`로 빈 archive를 금지한다. release commit 이후 source 또는 `Cargo.lock`이 바뀌면 해당 묶음은 무효다. |
| `MPC-TDF-010` | v1 release target은 macOS arm64와 Linux x86_64다. 각 target은 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, install/update/doctor, SQLite WAL, path protection, migration, purge를 검증한다. Windows와 다른 architecture는 v1 release claim에 포함하지 않는다. |

이 보충 계약은 외부 웹 freshness, encrypted store, multi-project, planning TUI, 의미 품질 점수, host model 선택을 추가하지 않는다.
