use crate::planning::{domain::*, engine::*};
use crate::planning_audit_support::with_full_audit_input;
use crate::planning_support::{question, start_core};

#[test]
fn full_audit_combinations_are_exact_and_atomic() {
    let (mut core, state, work) = with_full_audit_input(true, None, false);
    let combinations = [
        (AuditReadiness::RequestFullAudit, false, false),
        (AuditReadiness::Ready, true, false),
        (AuditReadiness::Ready, false, true),
        (AuditReadiness::Continue, false, false),
        (AuditReadiness::Continue, true, true),
    ];
    for (index, (readiness, has_ops, has_question)) in combinations.into_iter().enumerate() {
        let before = core.state(&state.session_id).unwrap().clone();
        let event_count = core.events().len();
        let entity_ops = if has_ops {
            vec![EntityOp::Create {
                temp_ref: format!("invalid-{index}"),
                body: EntityBody::Constraint {
                    statement: "추가 제약".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }]
        } else {
            Vec::new()
        };
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: work.work_item_id.clone(),
            mode: AuditMode::Full,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash.clone(),
            readiness,
            next_question: has_question.then_some(question()),
            entity_ops,
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        });
        assert!(
            matches!(result, Err(CoreError::ProposalSchemaInvalid(_))),
            "{index}"
        );
        assert_eq!(core.events().len(), event_count, "{index}");
        assert_eq!(core.state(&state.session_id).unwrap(), &before, "{index}");
    }
}

#[test]
fn delta_valid_and_invalid_combinations_are_table_driven() {
    for (readiness, has_question) in [
        (AuditReadiness::Continue, true),
        (AuditReadiness::RequestFullAudit, false),
    ] {
        let (mut core, state) = start_core();
        let work = state.required_model_action.clone().unwrap();
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness,
            next_question: has_question.then_some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        });
        assert!(result.is_ok(), "{readiness:?}");
    }
    for (readiness, next_question, review) in [
        (AuditReadiness::Ready, None, None),
        (AuditReadiness::Continue, None, None),
        (AuditReadiness::RequestFullAudit, Some(question()), None),
        (
            AuditReadiness::Continue,
            Some(question()),
            Some(CounterexampleReview::performed()),
        ),
    ] {
        let (mut core, state) = start_core();
        let work = state.required_model_action.clone().unwrap();
        let before = state.clone();
        let events = core.events().len();
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness,
            next_question,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: review,
        });
        assert!(matches!(result, Err(CoreError::ProposalSchemaInvalid(_))));
        assert_eq!(core.events().len(), events);
        assert_eq!(core.state(&state.session_id).unwrap(), &before);
    }
}

#[test]
fn full_continue_question_or_operations_and_ready_paths_are_valid() {
    let (mut question_core, question_state, question_work) =
        with_full_audit_input(true, None, false);
    let question_result = question_core
        .apply_audit(AuditCommand {
            session_id: question_state.session_id.clone(),
            expected_revision: question_state.revision,
            work_item_id: question_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: question_work.base_revision,
            base_domain_revision: question_work.base_domain_revision,
            input_hash: question_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    assert!(question_result.state.pending_question.is_some());

    let (mut ops_core, ops_state, ops_work) = with_full_audit_input(true, None, false);
    let ops_result = ops_core
        .apply_audit(AuditCommand {
            session_id: ops_state.session_id.clone(),
            expected_revision: ops_state.revision,
            work_item_id: ops_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: ops_work.base_revision,
            base_domain_revision: ops_work.base_domain_revision,
            input_hash: ops_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "extra".to_string(),
                body: EntityBody::Constraint {
                    statement: "추가".to_string(),
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
    assert_eq!(ops_result.state.phase, LifecyclePhase::Interview);

    let (mut ready_core, ready_state, ready_work) = with_full_audit_input(true, None, false);
    let ready = ready_core
        .apply_audit(AuditCommand {
            session_id: ready_state.session_id.clone(),
            expected_revision: ready_state.revision,
            work_item_id: ready_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: ready_work.base_revision,
            base_domain_revision: ready_work.base_domain_revision,
            input_hash: ready_work.input_hash,
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    assert_eq!(ready.state.phase, LifecyclePhase::Specification);
}

#[test]
fn blocking_counterexample_requires_and_accepts_matching_blocker_op() {
    let finding = CounterexampleFinding {
        statement: "반례".to_string(),
        result: CounterexampleResult::Blocking,
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
    };
    let (mut invalid_core, invalid_state, invalid_work) = with_full_audit_input(true, None, false);
    let invalid_before = invalid_state.clone();
    let invalid_events = invalid_core.events().len();
    let invalid = invalid_core.apply_audit(AuditCommand {
        session_id: invalid_state.session_id.clone(),
        expected_revision: invalid_state.revision,
        work_item_id: invalid_work.work_item_id,
        mode: AuditMode::Full,
        base_revision: invalid_work.base_revision,
        base_domain_revision: invalid_work.base_domain_revision,
        input_hash: invalid_work.input_hash,
        readiness: AuditReadiness::Continue,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview {
            performed: true,
            challenged_entity_ids: Vec::new(),
            findings: vec![finding.clone()],
        }),
    });
    assert!(matches!(invalid, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(invalid_core.events().len(), invalid_events);
    assert_eq!(
        invalid_core.state(&invalid_state.session_id).unwrap(),
        &invalid_before
    );

    let (mut valid_core, valid_state, valid_work) = with_full_audit_input(true, None, false);
    let valid = valid_core
        .apply_audit(AuditCommand {
            session_id: valid_state.session_id.clone(),
            expected_revision: valid_state.revision,
            work_item_id: valid_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: valid_work.base_revision,
            base_domain_revision: valid_work.base_domain_revision,
            input_hash: valid_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: vec![BlockerOp::Create {
                temp_ref: "counterexample".to_string(),
                kind: BlockerKind::Contradiction,
                severity: BlockerSeverity::Blocking,
                statement: finding.statement.clone(),
                source_refs: finding.source_refs.clone(),
            }],
            counterexample_review: Some(CounterexampleReview {
                performed: true,
                challenged_entity_ids: Vec::new(),
                findings: vec![finding],
            }),
        })
        .unwrap();
    assert_eq!(valid.state.phase, LifecyclePhase::Interview);
    assert!(valid.state.has_blocking_blocker());
}
