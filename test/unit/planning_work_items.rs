use crate::planning::domain::ModelActionKind;
use crate::planning::engine::work_item;
use crate::planning_support::{generated_spec_core, start_core};

#[test]
fn semantic_input_hash_is_session_independent_but_work_item_identity_is_not() {
    let (_, state) = start_core();
    let first = work_item(&state, ModelActionKind::DeltaAudit);

    let mut other_session = state.clone();
    other_session.session_id = "pln_other".to_string();
    let second = work_item(&other_session, ModelActionKind::DeltaAudit);
    assert_eq!(first.input_hash, second.input_hash);
    assert_ne!(first.work_item_id, second.work_item_id);

    let mut other_revision = state.clone();
    other_revision.revision = 9;
    other_revision.domain_revision = 4;
    other_revision.plan_revision = 2;
    let third = work_item(&other_revision, ModelActionKind::DeltaAudit);
    assert_eq!(first.input_hash, third.input_hash);
    assert_ne!(first.work_item_id, third.work_item_id);

    let mut changed_context = state;
    changed_context.transcript.initial_request.push_str(" 변경");
    let changed = work_item(&changed_context, ModelActionKind::DeltaAudit);
    assert_ne!(first.input_hash, changed.input_hash);
    assert_ne!(first.work_item_id, changed.work_item_id);
}

#[test]
fn question_authoring_is_present_only_for_audit_contexts() {
    let (_, state) = start_core();
    let audit = work_item(&state, ModelActionKind::DeltaAudit);
    assert_eq!(
        audit.context["question_authoring"]["version"],
        "megara.question-authoring/v1"
    );
    assert_eq!(
        audit.context["question_authoring"]["rules"]
            .as_array()
            .unwrap()
            .len(),
        7
    );

    let spec = work_item(&state, ModelActionKind::GenerateSpec);
    assert!(spec.context.get("question_authoring").is_none());
    assert!(spec.context.get("latest_answer").is_some());
}

#[test]
fn edge_only_context_changes_change_the_audit_input_hash() {
    let (_, state) = generated_spec_core();
    let with_edges = work_item(&state, ModelActionKind::DeltaAudit);
    let mut without_edges = state;
    without_edges.entities.edges.clear();
    let without_edges = work_item(&without_edges, ModelActionKind::DeltaAudit);
    assert_ne!(with_edges.input_hash, without_edges.input_hash);
    assert!(!with_edges.context["current_edges"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(without_edges.context["current_edges"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn current_edges_are_sorted_before_semantic_hashing() {
    let (_, state) = generated_spec_core();
    let ordered = work_item(&state, ModelActionKind::DeltaAudit);
    let mut reversed_state = state;
    reversed_state.entities.edges.reverse();
    let reversed = work_item(&reversed_state, ModelActionKind::DeltaAudit);
    assert_eq!(ordered.input_hash, reversed.input_hash);
    assert_eq!(
        ordered.context["current_edges"],
        reversed.context["current_edges"]
    );
}
