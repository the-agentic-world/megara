use crate::planning::canonical::canonical_hash;
use crate::planning::{domain::*, engine::*};

pub(crate) fn start_core() -> (InMemoryPlanningCore, PlanningState) {
    let mut core = InMemoryPlanningCore::default();
    let result = core
        .start(StartCommand {
            session_id: Some("pln_1".to_string()),
            project_id: "prj_1".to_string(),
            request: "기획 상태를 저장한다.".to_string(),
            title: None,
        })
        .unwrap();
    (core, result.state)
}

pub(crate) fn snapshot(evidence_hash: &str) -> RepoEvidenceSnapshot {
    RepoEvidenceSnapshot {
        evidence_hash: evidence_hash.to_string(),
        head_oid: None,
        head_ref: None,
        dirty: false,
        status_hash: format!("{evidence_hash}-status"),
        cited_files_hash: format!("{evidence_hash}-files"),
        evidence: vec![
            EvidenceRecord {
                evidence_id: "EVID-001".to_string(),
                path: "src/lib.rs".to_string(),
                ranges: Vec::new(),
                size: 1,
                sha256: "sha256:evidence-file-1".to_string(),
                tracked: true,
                captured_at: "unix-nanos:1".to_string(),
            },
            EvidenceRecord {
                evidence_id: "EVID-002".to_string(),
                path: "src/main.rs".to_string(),
                ranges: Vec::new(),
                size: 1,
                sha256: "sha256:evidence-file-2".to_string(),
                tracked: true,
                captured_at: "unix-nanos:1".to_string(),
            },
        ],
    }
}

pub(crate) fn question() -> QuestionProposal {
    QuestionProposal {
        context: "결정 배경입니다.".to_string(),
        question: "어떤 결과를 원하시나요?".to_string(),
        why_it_matters: "결과에 따라 명세가 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Freeform {
            freeform_hint: "원하는 결과를 설명해 주세요.".to_string(),
        },
    }
}

pub(crate) fn required_entity_ops() -> (Vec<EntityOp>, Vec<EdgeOp>) {
    let source = || {
        vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }]
    };
    let entities = vec![
        EntityOp::Create {
            temp_ref: "problem".to_string(),
            body: EntityBody::Problem {
                statement: "문제를 설명한다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "outcome".to_string(),
            body: EntityBody::Outcome {
                statement: "결과를 설명한다.".to_string(),
                observable_result: "검토 가능한 결과가 있다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "requirement".to_string(),
            body: EntityBody::Requirement {
                statement: "요구사항을 보존한다.".to_string(),
                priority: RequirementPriority::Must,
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "non_goal".to_string(),
            body: EntityBody::NonGoal {
                statement: "구현 실행은 하지 않는다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "boundary".to_string(),
            body: EntityBody::DecisionBoundary {
                autonomous_scope: vec!["구조 검증".to_string()],
                requires_user_approval: vec!["승인".to_string()],
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "criterion".to_string(),
            body: EntityBody::AcceptanceCriterion {
                statement: "요구사항을 확인할 수 있다.".to_string(),
            },
            source_refs: source(),
        },
    ];
    let edges = vec![EdgeOp::Add {
        kind: EdgeKind::HasAcceptanceCriterion,
        from: AuditEndpoint::TempRef {
            temp_ref: "requirement".to_string(),
        },
        to: AuditEndpoint::TempRef {
            temp_ref: "criterion".to_string(),
        },
        source_refs: source(),
    }];
    (entities, edges)
}

pub(crate) fn generated_spec_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, full) = full_audit_core();
    let audit_input_hash = full.full_audit.as_ref().unwrap().input_hash.clone();
    let generated = core
        .generate_spec(SpecCandidateCommand {
            session_id: full.session_id.clone(),
            expected_revision: full.revision,
            candidate: SpecCandidate {
                candidate_id: "cand_spec".to_string(),
                created_event_seq: full.revision + 1,
                created_ordinal: 0,
                base_domain_revision: full.domain_revision,
                audit_input_hash,
                semantic_hash: canonical_hash(&serde_json::json!({"title":"spec"})),
                entity_refs: Vec::new(),
                content: serde_json::json!({"title":"spec"}),
                stale: false,
            },
        })
        .unwrap();
    (core, generated.state)
}

pub(crate) fn full_audit_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, state) = start_core();
    let evidence = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            snapshot: snapshot("sha256:test-evidence"),
        })
        .unwrap();
    let state = match evidence {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("initial evidence must change state"),
    };
    let work_item = state.required_model_action.clone().unwrap();
    let (entity_ops, edge_ops) = required_entity_ops();
    let audit = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops,
            edge_ops,
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let full_work = audit.state.required_model_action.clone().unwrap();
    let full = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: audit.state.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash.clone(),
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    (core, full.state)
}

pub(crate) fn approved_spec_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, generated) = generated_spec_core();
    let semantic_hash = generated
        .spec
        .current_candidate
        .as_ref()
        .unwrap()
        .semantic_hash
        .clone();
    let approved = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash,
            base_revision: generated.domain_revision,
        })
        .unwrap();
    (core, approved.state)
}

pub(crate) fn completed_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, approved) = approved_spec_core();
    let plan_input_hash = approved
        .required_model_action
        .as_ref()
        .unwrap()
        .input_hash
        .clone();
    let spec_approval = approved.spec.approval.as_ref().unwrap();
    let requirement = approved.entities.current_requirements()[0];
    let criterion = approved.entities.current_acceptance_criteria()[0];
    let content = serde_json::json!({
        "baseline": {
            "commands": ["cargo test"],
            "known_failure_policy": "stop"
        },
        "steps": [{
            "temp_ref": "step-1",
            "objective": "검증 가능한 계획을 만든다.",
            "requirement_refs": [{
                "id": requirement.entity_id,
                "revision": requirement.revision
            }],
            "depends_on": [],
            "change_surface": ["src"],
            "risks": [],
            "rollback_or_recovery": "이전 상태로 복구한다."
        }],
        "verifications": [{
            "temp_ref": "verify-1",
            "acceptance_criterion_ref": {
                "id": criterion.entity_id,
                "revision": criterion.revision
            },
            "plan_step_refs": ["step-1"],
            "method": "command",
            "procedure": "cargo test",
            "expected_result": "통과한다."
        }],
        "plan_risks": []
    });
    let plan_hash = canonical_hash(&content);
    let generated = core
        .generate_plan(PlanCandidateCommand {
            session_id: approved.session_id.clone(),
            expected_revision: approved.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                created_event_seq: approved.revision + 1,
                created_ordinal: 0,
                base_plan_revision: approved.plan_revision,
                plan_input_hash,
                semantic_hash: plan_hash.clone(),
                spec_candidate_id: spec_approval.candidate_id.clone(),
                spec_semantic_hash: spec_approval.semantic_hash.clone(),
                content,
                stale: false,
            },
        })
        .unwrap();
    let complete = core
        .approve_plan(ApprovalCommand {
            session_id: generated.state.session_id.clone(),
            expected_revision: generated.state.revision,
            candidate_id: "cand_plan".to_string(),
            semantic_hash: plan_hash,
            base_revision: generated.state.plan_revision,
        })
        .unwrap();
    (core, complete.state)
}
