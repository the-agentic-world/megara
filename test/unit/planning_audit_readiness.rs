use crate::planning::{domain::*, engine::*};
use crate::planning_audit_support::with_full_audit_input;
use crate::planning_support::{question, start_core};

#[test]
fn readiness_one_missing_entity_conditions_each_block_full_specification() {
    for missing in [
        "problem",
        "outcome",
        "requirement",
        "non_goal",
        "boundary",
        "criterion",
    ] {
        let (mut core, state, work) = with_full_audit_input(true, Some(missing), false);
        let before = state.clone();
        let events = core.events().len();
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Full,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        });
        assert!(
            matches!(result, Err(CoreError::ProposalSchemaInvalid(_))),
            "{missing}"
        );
        assert_eq!(core.events().len(), events, "{missing}");
        assert_eq!(core.state(&state.session_id).unwrap(), &before, "{missing}");
    }
}

#[test]
fn readiness_blocker_evidence_review_and_binding_conditions_are_independent() {
    let (mut blocked_core, blocked_state, blocked_work) = with_full_audit_input(true, None, true);
    let blocked_before = blocked_state.clone();
    let blocked_events = blocked_core.events().len();
    let blocked = blocked_core.apply_audit(AuditCommand {
        session_id: blocked_state.session_id.clone(),
        expected_revision: blocked_state.revision,
        work_item_id: blocked_work.work_item_id,
        mode: AuditMode::Full,
        base_revision: blocked_work.base_revision,
        base_domain_revision: blocked_work.base_domain_revision,
        input_hash: blocked_work.input_hash,
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview::performed()),
    });
    assert!(matches!(blocked, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(blocked_core.events().len(), blocked_events);
    assert_eq!(
        blocked_core.state(&blocked_state.session_id).unwrap(),
        &blocked_before
    );

    let (mut evidence_core, evidence_state, evidence_work) =
        with_full_audit_input(false, None, false);
    let evidence_before = evidence_state.clone();
    let evidence_events = evidence_core.events().len();
    let evidence = evidence_core.apply_audit(AuditCommand {
        session_id: evidence_state.session_id.clone(),
        expected_revision: evidence_state.revision,
        work_item_id: evidence_work.work_item_id,
        mode: AuditMode::Full,
        base_revision: evidence_work.base_revision,
        base_domain_revision: evidence_work.base_domain_revision,
        input_hash: evidence_work.input_hash,
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview::performed()),
    });
    assert!(matches!(evidence, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(evidence_core.events().len(), evidence_events);
    assert_eq!(
        evidence_core.state(&evidence_state.session_id).unwrap(),
        &evidence_before
    );

    let (mut review_core, review_state, review_work) = with_full_audit_input(true, None, false);
    let review_before = review_state.clone();
    let review_events = review_core.events().len();
    let review = review_core.apply_audit(AuditCommand {
        session_id: review_state.session_id.clone(),
        expected_revision: review_state.revision,
        work_item_id: review_work.work_item_id,
        mode: AuditMode::Full,
        base_revision: review_work.base_revision,
        base_domain_revision: review_work.base_domain_revision,
        input_hash: review_work.input_hash,
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: None,
    });
    assert!(matches!(review, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(review_core.events().len(), review_events);
    assert_eq!(
        review_core.state(&review_state.session_id).unwrap(),
        &review_before
    );

    let (mut binding_core, binding_state, binding_work) = with_full_audit_input(true, None, false);
    let binding_before = binding_state.clone();
    let binding_events = binding_core.events().len();
    let binding = binding_core.apply_audit(AuditCommand {
        session_id: binding_state.session_id.clone(),
        expected_revision: binding_state.revision,
        work_item_id: binding_work.work_item_id,
        mode: AuditMode::Full,
        base_revision: binding_work.base_revision,
        base_domain_revision: binding_work.base_domain_revision,
        input_hash: "sha256:wrong-input".to_string(),
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview::performed()),
    });
    assert!(matches!(binding, Err(CoreError::ProposalBaseMismatch)));
    assert_eq!(binding_core.events().len(), binding_events);
    assert_eq!(
        binding_core.state(&binding_state.session_id).unwrap(),
        &binding_before
    );
}

#[test]
fn readiness_requires_current_requirement_to_acceptance_criterion_edge() {
    let (mut core, state, work) = with_full_audit_input(true, Some("edge"), false);
    assert!(state.entities.current("REQ-001").is_some());
    assert!(state.entities.current("AC-001").is_some());
    let before = state.clone();
    let events = core.events().len();
    let result = core.apply_audit(AuditCommand {
        session_id: state.session_id.clone(),
        expected_revision: state.revision,
        work_item_id: work.work_item_id,
        mode: AuditMode::Full,
        base_revision: work.base_revision,
        base_domain_revision: work.base_domain_revision,
        input_hash: work.input_hash,
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview::performed()),
    });
    assert!(matches!(result, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(core.events().len(), events);
    assert_eq!(core.state(&state.session_id).unwrap(), &before);
}

#[test]
fn pending_question_directly_fails_the_no_pending_readiness_condition() {
    let (_core, state, work) = with_full_audit_input(true, None, false);
    let mut pending_state = state.clone();
    pending_state.pending_question = Some(PendingQuestion {
        question_id: "qst-readiness-only".to_string(),
        created_event_seq: pending_state.revision,
        created_ordinal: 0,
        based_on_revision: pending_state.revision,
        proposal: question(),
    });
    let gate = crate::planning::engine::compute_readiness_gate(
        &pending_state,
        &work.input_hash,
        Some(&CounterexampleReview::performed()),
    );
    assert!(gate.problem);
    assert!(gate.outcome);
    assert!(gate.requirement);
    assert!(gate.non_goal);
    assert!(gate.decision_boundary);
    assert!(gate.acceptance_criteria);
    assert!(gate.no_blocking_blockers);
    assert!(gate.evidence_current);
    assert!(gate.audit_input_current);
    assert!(gate.counterexample_review);
    assert!(!gate.no_pending_question);
    assert!(pending_state.pending_question.is_some());
}

#[test]
fn pending_question_prevents_another_audit_work_item() {
    let (mut core, state) = start_core();
    let work = state.required_model_action.clone().unwrap();
    let pending = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let before = pending.state.clone();
    let events = core.events().len();
    let result = core.apply_audit(AuditCommand {
        session_id: before.session_id.clone(),
        expected_revision: before.revision,
        work_item_id: "wrk_full_not_available".to_string(),
        mode: AuditMode::Full,
        base_revision: before.revision + 1,
        base_domain_revision: before.domain_revision,
        input_hash: "sha256:not-available".to_string(),
        readiness: AuditReadiness::Ready,
        next_question: None,
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: Some(CounterexampleReview::performed()),
    });
    assert!(matches!(result, Err(CoreError::ModelActionMismatch)));
    assert_eq!(core.events().len(), events);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}

#[test]
fn audit_binding_mismatch_matrix_is_atomic() {
    let (mut core, state, work) = with_full_audit_input(true, None, false);
    let mismatches = [
        (
            "work item",
            "wrong".to_string(),
            work.base_revision,
            work.base_domain_revision,
            work.input_hash.clone(),
            AuditMode::Full,
        ),
        (
            "base revision",
            work.work_item_id.clone(),
            work.base_revision + 1,
            work.base_domain_revision,
            work.input_hash.clone(),
            AuditMode::Full,
        ),
        (
            "domain revision",
            work.work_item_id.clone(),
            work.base_revision,
            work.base_domain_revision + 1,
            work.input_hash.clone(),
            AuditMode::Full,
        ),
        (
            "input hash",
            work.work_item_id.clone(),
            work.base_revision,
            work.base_domain_revision,
            "sha256:wrong".to_string(),
            AuditMode::Full,
        ),
        (
            "kind",
            work.work_item_id.clone(),
            work.base_revision,
            work.base_domain_revision,
            work.input_hash.clone(),
            AuditMode::Delta,
        ),
    ];
    for (label, id, base_revision, base_domain_revision, input_hash, mode) in mismatches {
        let before = core.state(&state.session_id).unwrap().clone();
        let events = core.events().len();
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: id,
            mode,
            base_revision,
            base_domain_revision,
            input_hash,
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        });
        assert!(
            matches!(result, Err(CoreError::ProposalBaseMismatch)),
            "{label}"
        );
        assert_eq!(core.events().len(), events, "{label}");
        assert_eq!(core.state(&state.session_id).unwrap(), &before, "{label}");
    }
}
