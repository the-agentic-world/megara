use std::fs;

use crate::planning::store::PlanningStore;
use crate::planning_artifact_support::{plan_proposal_for, request, ArtifactHarness};
use serde_json::json;

#[test]
fn stale_evidence_blocks_spec_and_plan_generate_and_approve_with_zero_delta() {
    let mut spec_generate = ArtifactHarness::new();
    let before = spec_generate.status_state();
    let events = event_count(&spec_generate);
    change_cited_file(&spec_generate);
    let spec_request = spec_generate.spec_request("cmd-stale-spec-generate", false);
    let response = spec_generate.service.handle_request(spec_request);
    assert_stale(&response, &mut spec_generate, &before, events);

    let mut spec_approve = ArtifactHarness::new();
    let generated = spec_approve.generate_spec("cmd-stale-spec-approve-generate");
    let before = spec_approve.status_state();
    let events = event_count(&spec_approve);
    change_cited_file(&spec_approve);
    let candidate = before["spec"]["current_candidate"].clone();
    let response = spec_approve.service.handle_user_request(request(
        "planning.spec.approve",
        "cmd-stale-spec-approve",
        Some(&spec_approve.session_id),
        Some(generated["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "semantic_hash":candidate["semantic_hash"],
            "base_domain_revision":candidate["base_domain_revision"]
        }),
    ));
    assert_stale(&response, &mut spec_approve, &before, events);

    let mut plan_generate = ArtifactHarness::new();
    plan_generate.generate_spec("cmd-stale-plan-spec-generate");
    plan_generate.approve_spec("cmd-stale-plan-spec-approve");
    let before = plan_generate.status_state();
    let events = event_count(&plan_generate);
    change_cited_file(&plan_generate);
    let proposal = plan_proposal_for(&before);
    let response = plan_generate.service.handle_request(request(
        "planning.plan.generate",
        "cmd-stale-plan-generate",
        Some(&plan_generate.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({"proposal":proposal,"projection_policy":{"force":false}}),
    ));
    assert_stale(&response, &mut plan_generate, &before, events);

    let mut plan_approve = ArtifactHarness::new();
    plan_approve.generate_spec("cmd-stale-plan-approve-spec-generate");
    plan_approve.approve_spec("cmd-stale-plan-approve-spec-approve");
    let generated = plan_approve.generate_plan("cmd-stale-plan-approve-generate");
    let before = plan_approve.status_state();
    let events = event_count(&plan_approve);
    change_cited_file(&plan_approve);
    let candidate = before["plan"]["current_candidate"].clone();
    let response = plan_approve.service.handle_user_request(request(
        "planning.plan.approve",
        "cmd-stale-plan-approve",
        Some(&plan_approve.session_id),
        Some(generated["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "semantic_hash":candidate["semantic_hash"],
            "base_plan_revision":candidate["base_plan_revision"]
        }),
    ));
    assert_stale(&response, &mut plan_approve, &before, events);
}

#[test]
fn model_authority_cannot_approve_spec_or_plan() {
    let mut spec = ArtifactHarness::new();
    spec.generate_spec("cmd-model-spec-generate");
    let before = spec.status_state();
    let events = event_count(&spec);
    let candidate = before["spec"]["current_candidate"].clone();
    let response = spec.service.handle_request(request(
        "planning.spec.approve",
        "cmd-model-spec-approve",
        Some(&spec.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "semantic_hash":candidate["semantic_hash"],
            "base_domain_revision":candidate["base_domain_revision"]
        }),
    ));
    assert_eq!(response["error"]["code"], "USER_ENTRYPOINT_REQUIRED");
    assert_unchanged(&mut spec, &before, events);

    spec.approve_spec("cmd-model-spec-approve-user");
    spec.generate_plan("cmd-model-plan-generate");
    let before = spec.status_state();
    let events = event_count(&spec);
    let candidate = before["plan"]["current_candidate"].clone();
    let response = spec.service.handle_request(request(
        "planning.plan.approve",
        "cmd-model-plan-approve",
        Some(&spec.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "semantic_hash":candidate["semantic_hash"],
            "base_plan_revision":candidate["base_plan_revision"]
        }),
    ));
    assert_eq!(response["error"]["code"], "USER_ENTRYPOINT_REQUIRED");
    assert_unchanged(&mut spec, &before, events);
}

fn change_cited_file(harness: &ArtifactHarness) {
    fs::write(
        harness.directory.path().join("src/main.rs"),
        "fn main() { stale(); }\n",
    )
    .unwrap();
}

fn event_count(harness: &ArtifactHarness) -> u64 {
    PlanningStore::open_project(harness.directory.path())
        .unwrap()
        .event_count(&harness.session_id)
        .unwrap()
}

fn assert_stale(
    response: &serde_json::Value,
    harness: &mut ArtifactHarness,
    before: &serde_json::Value,
    events: u64,
) {
    assert_eq!(response["error"]["code"], "EVIDENCE_STALE");
    assert_unchanged(harness, before, events);
}

fn assert_unchanged(harness: &mut ArtifactHarness, before: &serde_json::Value, events: u64) {
    assert_eq!(harness.status_state(), *before);
    assert_eq!(event_count(harness), events);
}
