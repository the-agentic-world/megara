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
    let generated = core
        .generate_spec(SpecCandidateCommand {
            session_id: state.session_id.clone(),
            expected_revision: full.state.revision,
            candidate: SpecCandidate {
                candidate_id: "cand_spec".to_string(),
                base_domain_revision: full.state.domain_revision,
                audit_input_hash: full_work.input_hash,
                semantic_hash: "sha256:spec".to_string(),
                entity_refs: Vec::new(),
                content: serde_json::json!({"title":"spec"}),
                stale: false,
            },
        })
        .unwrap();
    (core, generated.state)
}

pub(crate) fn approved_spec_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, generated) = generated_spec_core();
    let approved = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash: "sha256:spec".to_string(),
            base_revision: generated.domain_revision,
        })
        .unwrap();
    (core, approved.state)
}

pub(crate) fn completed_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, approved) = approved_spec_core();
    let generated = core
        .generate_plan(PlanCandidateCommand {
            session_id: approved.session_id.clone(),
            expected_revision: approved.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                base_plan_revision: approved.plan_revision,
                plan_input_hash: "sha256:plan-input".to_string(),
                semantic_hash: "sha256:plan".to_string(),
                spec_candidate_id: "cand_spec".to_string(),
                spec_semantic_hash: "sha256:spec".to_string(),
                content: serde_json::json!({"steps": []}),
                stale: false,
            },
        })
        .unwrap();
    let complete = core
        .approve_plan(ApprovalCommand {
            session_id: generated.state.session_id.clone(),
            expected_revision: generated.state.revision,
            candidate_id: "cand_plan".to_string(),
            semantic_hash: "sha256:plan".to_string(),
            base_revision: generated.state.plan_revision,
        })
        .unwrap();
    (core, complete.state)
}
