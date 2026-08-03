use std::fs;

use crate::planning::canonical::canonical_hash;
use crate::planning::domain::{
    EntityBody, EntityDisposition, EntityKind, EntityRecord, EntityValidity, SourceRef,
};
use crate::planning::engine::{
    plan_input_hash, spec_semantic_hash, validate_plan_content, CoreError,
};
use crate::planning::store::{normalized_state_hash, PlanningStore};
use crate::planning_artifact_support::{
    assert_ok, plan_proposal_for, request, spec_proposal_for, ArtifactHarness,
};
use crate::planning_support::{approved_spec_core, generated_spec_core};
use serde_json::{json, Value};
use uuid::Uuid;

#[test]
fn service_candidate_provenance_matches_generation_event_and_aliases_uuid_variants() {
    let mut harness = ArtifactHarness::new();
    let spec = harness.generate_spec("cmd-candidate-provenance-spec");
    harness.approve_spec("cmd-candidate-provenance-spec-approve");
    let plan = harness.generate_plan("cmd-candidate-provenance-plan");
    assert_eq!(
        spec["result"]["candidate"]["created_event_seq"],
        spec["revision"]
    );
    assert_eq!(
        plan["result"]["candidate"]["created_event_seq"],
        plan["revision"]
    );

    let first_store = PlanningStore::open_project(harness.directory.path()).unwrap();
    let first = first_store.current(&harness.session_id).unwrap().clone();
    let mut equivalent = first.clone();
    let spec_id = format!("spec_{}", Uuid::now_v7());
    let plan_id = format!("plan_{}", Uuid::now_v7());
    equivalent
        .spec
        .current_candidate
        .as_mut()
        .unwrap()
        .candidate_id = spec_id.clone();
    equivalent.spec.approval.as_mut().unwrap().candidate_id = spec_id.clone();
    equivalent
        .plan
        .current_candidate
        .as_mut()
        .unwrap()
        .candidate_id = plan_id;
    equivalent
        .plan
        .current_candidate
        .as_mut()
        .unwrap()
        .spec_candidate_id = spec_id;
    assert_eq!(
        normalized_state_hash(&first),
        normalized_state_hash(&equivalent)
    );
    assert_eq!(plan_input_hash(&first), plan_input_hash(&equivalent));
}

#[test]
fn spec_wire_requires_exact_current_refs_and_unknown_fields() {
    let mut harness = ArtifactHarness::new();
    let before = harness.status_state();
    let work = before["required_model_action"].clone();
    let mut missing = spec_proposal_for(&before, &work);
    missing["requirement_refs"] = json!([]);
    let response = harness.service.handle_request(request(
        "planning.spec.generate",
        "cmd-spec-missing-ref",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({"proposal":missing,"projection_policy":{"force":false}}),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    let after = harness.status_state();
    assert_eq!(after, before);

    let mut unknown = spec_proposal_for(&after, &after["required_model_action"]);
    unknown["unexpected"] = json!(true);
    let response = harness.service.handle_request(request(
        "planning.spec.generate",
        "cmd-spec-unknown",
        Some(&harness.session_id),
        Some(after["revision"].as_u64().unwrap()),
        json!({"proposal":unknown,"projection_policy":{"force":false}}),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    assert_eq!(harness.status_state(), after);
}

#[test]
fn spec_reference_matrix_rejects_duplicate_wrong_historical_and_unknown_refs_atomically() {
    let mut harness = historical_spec_harness();
    let before = harness.status_state();
    let valid = spec_proposal_for(&before, &before["required_model_action"]);
    let requirement = valid["requirement_refs"][0].clone();
    let problem = valid["problem_ref"].clone();
    let historical = json!({
        "id":requirement["id"],
        "revision":requirement["revision"].as_u64().unwrap() - 1
    });
    let unknown = json!({
        "id":requirement["id"],
        "revision":requirement["revision"].as_u64().unwrap() + 1
    });
    let mut duplicate = valid.clone();
    duplicate["requirement_refs"] = json!([requirement.clone(), requirement]);
    let mut wrong_kind = valid.clone();
    wrong_kind["problem_ref"] = requirement;
    wrong_kind["requirement_refs"] = json!([problem]);
    let mut stale = valid.clone();
    stale["requirement_refs"] = json!([historical]);
    let mut unknown_revision = valid.clone();
    unknown_revision["requirement_refs"] = json!([unknown]);
    let cases = [
        ("duplicate", duplicate),
        ("wrong-kind", wrong_kind),
        ("historical-or-unknown", stale),
        ("unknown-revision", unknown_revision),
    ];
    for (label, proposal) in cases {
        let events = event_count(&harness);
        let response = harness.service.handle_request(request(
            "planning.spec.generate",
            &format!("cmd-spec-reference-{label}"),
            Some(&harness.session_id),
            Some(before["revision"].as_u64().unwrap()),
            json!({"proposal":proposal,"projection_policy":{"force":false}}),
        ));
        assert_eq!(
            response["error"]["code"], "PROPOSAL_SCHEMA_INVALID",
            "{label}"
        );
        assert_eq!(harness.status_state(), before, "{label}");
        assert_eq!(event_count(&harness), events, "{label}");
    }
}

#[test]
fn plan_wire_rejects_unknown_and_structurally_invalid_content_atomically() {
    let mut harness = ArtifactHarness::new();
    harness.generate_spec("cmd-spec-for-plan");
    harness.approve_spec("cmd-approve-spec-for-plan");
    let before = harness.status_state();

    let mut unknown = plan_proposal_for(&before);
    unknown["unexpected"] = json!(true);
    let response = harness.service.handle_request(request(
        "planning.plan.generate",
        "cmd-plan-unknown",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({"proposal":unknown,"projection_policy":{"force":false}}),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    assert_eq!(harness.status_state(), before);

    let mut cycle = plan_proposal_for(&before);
    cycle["steps"][0]["depends_on"] = json!(["missing-step"]);
    let response = harness.service.handle_request(request(
        "planning.plan.generate",
        "cmd-plan-missing-dependency",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({"proposal":cycle,"projection_policy":{"force":false}}),
    ));
    assert_eq!(response["error"]["code"], "PROPOSAL_SCHEMA_INVALID");
    assert_eq!(harness.status_state(), before);
}

#[test]
fn spec_semantic_hash_normalizes_formatting_but_changes_with_meaning() {
    let (_core, state) = generated_spec_core();
    let first = json!({"title":"\nCanonical\r\ntitle \t\n"});
    let second = json!({"title":"Canonical\ntitle"});
    let changed = json!({"title":"Different title"});
    let content_space_changed = json!({"title":" Canonical\ntitle"});
    assert_eq!(
        spec_semantic_hash(&state, &first),
        spec_semantic_hash(&state, &second)
    );
    assert_ne!(
        spec_semantic_hash(&state, &first),
        spec_semantic_hash(&state, &changed)
    );
    assert_ne!(
        spec_semantic_hash(&state, &first),
        spec_semantic_hash(&state, &content_space_changed)
    );
}

#[test]
fn plan_structural_validator_allows_later_refs_and_rejects_cycles() {
    let (_core, state) = approved_spec_core();
    let requirement = state.entities.current_requirements()[0];
    let criterion = state.entities.current_acceptance_criteria()[0];
    let reference = json!({"id":requirement.entity_id,"revision":requirement.revision});
    let criterion_ref = json!({"id":criterion.entity_id,"revision":criterion.revision});
    let valid = plan_content(
        reference.clone(),
        criterion_ref.clone(),
        json!(["step-b", "step-c"]),
        json!([]),
    );
    assert!(validate_plan_content(&state, &valid).is_ok());

    let mut cycle = valid.clone();
    cycle["steps"][1]["depends_on"] = json!(["step-main"]);
    assert!(matches!(
        validate_plan_content(&state, &cycle),
        Err(CoreError::ProposalSchemaInvalid(_))
    ));
}

#[test]
fn plan_hash_keeps_dependency_sets_stable_but_preserves_step_order() {
    let (_core, state) = approved_spec_core();
    let requirement = state.entities.current_requirements()[0];
    let criterion = state.entities.current_acceptance_criteria()[0];
    let reference = json!({"id":requirement.entity_id,"revision":requirement.revision});
    let criterion_ref = json!({"id":criterion.entity_id,"revision":criterion.revision});
    let first = plan_content(
        reference.clone(),
        criterion_ref.clone(),
        json!(["step-b", "step-c"]),
        json!([]),
    );
    let mut dependency_order = first.clone();
    dependency_order["steps"][0]["depends_on"] = json!(["step-c", "step-b"]);
    assert_eq!(canonical_hash(&first), canonical_hash(&dependency_order));

    let mut step_order = first.clone();
    let steps = step_order["steps"].as_array_mut().unwrap();
    steps.swap(0, 1);
    assert_ne!(canonical_hash(&first), canonical_hash(&step_order));
}

#[test]
fn plan_structural_one_missing_matrix_rejects_each_traceability_gap() {
    let (_core, state) = approved_spec_core();
    let requirement = state.entities.current_requirements()[0];
    let criterion = state.entities.current_acceptance_criteria()[0];
    let reference = json!({"id":requirement.entity_id,"revision":requirement.revision});
    let criterion_ref = json!({"id":criterion.entity_id,"revision":criterion.revision});
    let valid = plan_content(
        reference.clone(),
        criterion_ref.clone(),
        json!([]),
        json!([]),
    );
    let mut orphan_step = valid.clone();
    orphan_step["steps"][0]["requirement_refs"] = json!([]);
    let mut requirement_gap_state = state.clone();
    requirement_gap_state
        .entities
        .insert(EntityRecord {
            entity_id: "REQ-002".to_string(),
            internal_uuid: "internal-req-002".to_string(),
            revision: 1,
            kind: EntityKind::Requirement,
            body: EntityBody::Requirement {
                statement: "second requirement".to_string(),
                priority: crate::planning::domain::RequirementPriority::Should,
            },
            disposition: EntityDisposition::Current,
            validity: EntityValidity::Valid,
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            created_event_seq: 1,
            created_ordinal: 20,
        })
        .unwrap();
    let uncovered_acceptance_criterion = valid.clone();
    let mut acceptance_gap_state = state.clone();
    acceptance_gap_state
        .entities
        .insert(EntityRecord {
            entity_id: "AC-002".to_string(),
            internal_uuid: "internal-ac-002".to_string(),
            revision: 1,
            kind: EntityKind::AcceptanceCriterion,
            body: EntityBody::AcceptanceCriterion {
                statement: "second criterion".to_string(),
            },
            disposition: EntityDisposition::Current,
            validity: EntityValidity::Valid,
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            created_event_seq: 1,
            created_ordinal: 21,
        })
        .unwrap();
    let mut broken_verification_link = valid.clone();
    broken_verification_link["verifications"][0]["plan_step_refs"] = json!(["missing"]);
    let mut duplicate_requirement_ref = valid.clone();
    duplicate_requirement_ref["steps"][0]["requirement_refs"] = json!([reference, reference]);
    assert_plan_invalid("orphan step", &state, &orphan_step);
    assert_plan_invalid("uncovered requirement", &requirement_gap_state, &valid);
    assert_plan_invalid(
        "uncovered acceptance criterion",
        &acceptance_gap_state,
        &uncovered_acceptance_criterion,
    );
    assert_plan_invalid(
        "broken verification link",
        &state,
        &broken_verification_link,
    );
    assert_plan_invalid(
        "duplicate requirement reference",
        &state,
        &duplicate_requirement_ref,
    );
}

fn assert_plan_invalid(
    name: &str,
    state: &crate::planning::domain::PlanningState,
    content: &Value,
) {
    assert!(
        matches!(
            validate_plan_content(state, content),
            Err(CoreError::ProposalSchemaInvalid(_))
        ),
        "case={name}"
    );
}

fn event_count(harness: &ArtifactHarness) -> u64 {
    PlanningStore::open_project(harness.directory.path())
        .unwrap()
        .event_count(&harness.session_id)
        .unwrap()
}

fn historical_spec_harness() -> ArtifactHarness {
    let mut harness = ArtifactHarness::new();
    let before = harness.status_state();
    fs::write(
        harness.directory.path().join("src/main.rs"),
        "fn main() { historical_reference(); }\n",
    )
    .unwrap();
    let refreshed = harness.service.handle_request(request(
        "planning.evidence.refresh",
        "cmd-spec-history-refresh",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({"citations":[{"temp_ref":"main","path":"src/main.rs","ranges":[],"claim":"historical reference source"}]}),
    ));
    assert_ok(&refreshed);
    let state = refreshed["result"]["state"].clone();
    let work = state["required_model_action"].clone();
    let requirement = entity_of_kind(&state, "requirement");
    let criterion = entity_of_kind(&state, "acceptance_criterion");
    let source = json!([{"kind":"initial_request","id":"request"}]);
    let delta = harness.service.handle_request(request(
        "planning.audit.apply",
        "cmd-spec-history-revise",
        Some(&harness.session_id),
        Some(refreshed["revision"].as_u64().unwrap()),
        json!({
            "mode":"delta",
            "proposal":{
                "schema":"megara.audit-proposal/v1","mode":"delta",
                "work_item_id":work["work_item_id"],"base_revision":work["base_revision"],
                "base_domain_revision":work["base_domain_revision"],"input_hash":work["input_hash"],
                "readiness":"request_full_audit","next_question":null,
                "entity_ops":[{"op":"revise","entity_id":requirement["entity_id"],"base_entity_revision":requirement["revision"],"body":{"statement":"revised requirement","priority":"must"},"source_refs":source.clone()}],
                "edge_ops":[{"op":"add","kind":"has_acceptance_criterion","from":{"entity_id":requirement["entity_id"],"revision":requirement["revision"].as_u64().unwrap()+1},"to":{"entity_id":criterion["entity_id"],"revision":criterion["revision"]},"source_refs":source}],
                "blocker_ops":[],"counterexample_review":null
            }
        }),
    ));
    assert_ok(&delta);
    let full_work = delta["result"]["state"]["required_model_action"].clone();
    let full = harness.service.handle_request(request(
        "planning.audit.apply",
        "cmd-spec-history-full",
        Some(&harness.session_id),
        Some(delta["revision"].as_u64().unwrap()),
        json!({
            "mode":"full",
            "proposal":{
                "schema":"megara.audit-proposal/v1","mode":"full",
                "work_item_id":full_work["work_item_id"],"base_revision":full_work["base_revision"],
                "base_domain_revision":full_work["base_domain_revision"],"input_hash":full_work["input_hash"],
                "readiness":"ready","next_question":null,"entity_ops":[],"edge_ops":[],"blocker_ops":[],
                "counterexample_review":{"performed":true,"challenged_entity_ids":[],"findings":[]}
            }
        }),
    ));
    assert_ok(&full);
    harness
}

fn entity_of_kind(state: &Value, kind: &str) -> Value {
    state["entities"]["revisions"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|records| records.as_array()?.last())
        .find(|record| record["kind"] == kind)
        .cloned()
        .unwrap()
}

fn plan_content(
    requirement: Value,
    criterion: Value,
    first_dependencies: Value,
    second_dependencies: Value,
) -> Value {
    json!({
        "baseline":{"commands":["cargo test"],"known_failure_policy":"stop"},
        "steps":[
            {"temp_ref":"step-main","objective":"main objective","requirement_refs":[requirement],"depends_on":first_dependencies,"change_surface":["src"],"risks":[],"rollback_or_recovery":"restore"},
            {"temp_ref":"step-b","objective":"second objective","requirement_refs":[requirement],"depends_on":second_dependencies,"change_surface":["src"],"risks":[],"rollback_or_recovery":"restore"},
            {"temp_ref":"step-c","objective":"third objective","requirement_refs":[requirement],"depends_on":[],"change_surface":["src"],"risks":[],"rollback_or_recovery":"restore"}
        ],
        "verifications":[{"temp_ref":"verify","acceptance_criterion_ref":criterion,"plan_step_refs":["step-main","step-b","step-c"],"method":"command","procedure":"cargo test","expected_result":"pass"}],
        "plan_risks":[]
    })
}
