use crate::planning::{domain::*, engine::*};
use crate::planning_support::*;

#[test]
fn start_creates_one_revision_and_one_model_action() {
    let (core, state) = start_core();
    assert_eq!(state.revision, 1);
    assert_eq!(core.events().len(), 1);
    assert_eq!(core.events()[0].seq, 1);
    assert_eq!(state.domain_revision, 1);
    assert_eq!(core.events()[0].domain_revision_after, 1);
    assert_eq!(
        state.required_model_action.as_ref().unwrap().base_revision,
        1
    );
    assert_eq!(
        state
            .required_model_action
            .as_ref()
            .unwrap()
            .base_domain_revision,
        1
    );
    assert!(state.derived().waiting_for_model);
    assert!(!state.derived().waiting_for_user);
}

#[test]
fn stale_answer_has_no_event_or_state_change() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.clone().unwrap();
    let audit = AuditCommand {
        session_id: state.session_id.clone(),
        expected_revision: state.revision,
        work_item_id: work_item.work_item_id,
        mode: AuditMode::Delta,
        base_revision: work_item.base_revision,
        base_domain_revision: work_item.base_domain_revision,
        input_hash: work_item.input_hash,
        readiness: AuditReadiness::Continue,
        next_question: Some(question()),
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: None,
    };
    let result = core.apply_audit(audit).unwrap();
    let before = result.state.clone();
    let event_count = core.events().len();
    let error = core
        .answer(AnswerCommand {
            session_id: "pln_1".to_string(),
            expected_revision: before.revision - 1,
            question_id: "qst_2".to_string(),
            based_on_revision: before.revision,
            text: "답".to_string(),
            selected_choice_ids: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::RevisionConflict { .. }));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state("pln_1").unwrap(), &before);
}

#[test]
fn pending_question_and_model_action_never_coexist() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.unwrap();
    let result = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    assert!(result.state.pending_question.is_some());
    assert!(result.state.required_model_action.is_none());
    assert!(result.state.assert_invariants().is_ok());
}

#[test]
fn core_does_not_enter_specification_from_self_attested_empty_gate() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.unwrap();
    let requested = core
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
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let before = requested.state.clone();
    let event_count = core.events().len();
    let full_work = before.required_model_action.clone().unwrap();
    let error = core
        .apply_audit(AuditCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash,
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::ProposalSchemaInvalid(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}

#[test]
fn source_rule_and_illegal_phase_failures_preserve_state() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.clone().unwrap();
    let before = state.clone();
    let event_count = core.events().len();
    let error = core
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
            entity_ops: vec![EntityOp::Create {
                temp_ref: "fact".to_string(),
                body: EntityBody::Fact {
                    statement: "비밀 사실".to_string(),
                    evidence_refs: vec!["EVID-001".to_string()],
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidRequest(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);

    let error = core
        .generate_spec(SpecCandidateCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            candidate: SpecCandidate {
                candidate_id: "cand_invalid".to_string(),
                base_domain_revision: 0,
                audit_input_hash: "sha256:none".to_string(),
                semantic_hash: "sha256:none".to_string(),
                entity_refs: Vec::new(),
                content: serde_json::json!({}),
                stale: false,
            },
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidPhase(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}

#[test]
fn requirement_cannot_use_historical_decision_source() {
    let (mut core, state) = start_core();
    let start_work = state.required_model_action.unwrap();
    let created = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: start_work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: start_work.base_revision,
            base_domain_revision: start_work.base_domain_revision,
            input_hash: start_work.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "decision".to_string(),
                body: EntityBody::Decision {
                    statement: "결정을 기록한다.".to_string(),
                    selected_option: "현재 선택".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let full_work = created.state.required_model_action.unwrap();
    let revised = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: created.state.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: vec![EntityOp::Revise {
                entity_id: "DEC-001".to_string(),
                base_entity_revision: 1,
                body: EntityBody::Decision {
                    statement: "결정을 다시 기록한다.".to_string(),
                    selected_option: "새 선택".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    let delta_work = revised.state.required_model_action.clone().unwrap();
    let before = revised.state.clone();
    let event_count = core.events().len();
    let error = core
        .apply_audit(AuditCommand {
            session_id: state.session_id,
            expected_revision: before.revision,
            work_item_id: delta_work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: delta_work.base_revision,
            base_domain_revision: delta_work.base_domain_revision,
            input_hash: delta_work.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "requirement".to_string(),
                body: EntityBody::Requirement {
                    statement: "옛 결정을 근거로 삼는다.".to_string(),
                    priority: RequirementPriority::Must,
                },
                source_refs: vec![SourceRef::Entity {
                    id: "DEC-001".to_string(),
                    revision: 1,
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidRequest(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state("pln_1").unwrap(), &before);
}
