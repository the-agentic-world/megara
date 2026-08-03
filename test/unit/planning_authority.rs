use super::planning_artifact_support::{request, ArtifactHarness};
use crate::planning::service::PlanningService;
use crate::planning::store::{EventActor, EventAdapter, PlanningStore};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn model_adapter_event_metadata_is_explicit_for_pi_and_codex() {
    let pi_dir = tempdir().unwrap();
    let mut pi = PlanningService::open_project(pi_dir.path()).unwrap();
    let pi_response = pi.handle_request(request(
        "planning.start",
        "cmd-pi-start",
        None,
        None,
        json!({"request":"Pi request"}),
    ));
    let pi_store = PlanningStore::open_project(pi_dir.path()).unwrap();
    let pi_event = pi_store
        .event_envelopes(pi_response["session_id"].as_str().unwrap())
        .unwrap()
        .remove(0);
    assert_eq!(pi_event.metadata.actor, EventActor::Model);
    assert_eq!(pi_event.metadata.adapter, EventAdapter::Pi);

    let codex_dir = tempdir().unwrap();
    let mut codex = PlanningService::open_project(codex_dir.path()).unwrap();
    let codex_response = codex.handle_codex_request(request(
        "planning.start",
        "cmd-codex-start",
        None,
        None,
        json!({"request":"Codex request"}),
    ));
    let codex_store = PlanningStore::open_project(codex_dir.path()).unwrap();
    let codex_event = codex_store
        .event_envelopes(codex_response["session_id"].as_str().unwrap())
        .unwrap()
        .remove(0);
    assert_eq!(codex_event.metadata.actor, EventActor::Model);
    assert_eq!(codex_event.metadata.adapter, EventAdapter::CodexMcp);
}

#[test]
fn pi_revision_and_export_are_user_entrypoint_only() {
    let mut harness = ArtifactHarness::new();
    harness.generate_spec("cmd-authority-spec-generate");
    let before = harness.status_state();
    let events = PlanningStore::open_project(harness.directory.path())
        .unwrap()
        .event_count(&harness.session_id)
        .unwrap();
    let candidate = before["spec"]["current_candidate"].clone();
    let revised = harness.service.handle_request(request(
        "planning.spec.revise",
        "cmd-pi-revise",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "text":"Pi must not revise through model RPC"
        }),
    ));
    assert_eq!(revised["error"]["code"], "USER_ENTRYPOINT_REQUIRED");
    assert_eq!(harness.status_state(), before);
    assert_eq!(event_count(&harness), events);

    harness.approve_spec("cmd-authority-spec-approve");
    harness.generate_plan("cmd-authority-plan-generate");
    harness.approve_plan("cmd-authority-plan-approve");
    let before = harness.status_state();
    let events = event_count(&harness);
    let output = harness.directory.path().join("pi-export");
    let pi_export = harness.service.handle_request(request(
        "planning.export",
        "cmd-pi-export",
        Some(&harness.session_id),
        None,
        json!({"out":output,"format":"bundle","include_transcript":false,"force":false}),
    ));
    assert_eq!(pi_export["error"]["code"], "USER_ENTRYPOINT_REQUIRED");
    assert!(!output.exists());
    assert_eq!(harness.status_state(), before);
    assert_eq!(event_count(&harness), events);
}

#[test]
fn codex_model_revision_is_distinct_and_carries_codex_metadata() {
    let mut harness = ArtifactHarness::new();
    harness.generate_spec("cmd-codex-revise-generate");
    let before = harness.status_state();
    let candidate = before["spec"]["current_candidate"].clone();
    let response = harness.service.handle_codex_request(request(
        "planning.spec.revise",
        "cmd-codex-revise",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "text":"Codex MCP revision remains a model operation only where policy allows it"
        }),
    ));
    assert_eq!(response["ok"], true, "response={response}");
    let store = PlanningStore::open_project(harness.directory.path()).unwrap();
    let event = store
        .event_envelopes(&harness.session_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(event.metadata.actor, EventActor::Model);
    assert_eq!(event.metadata.adapter, EventAdapter::CodexMcp);
}

#[test]
fn codex_prompt_authority_is_user_codex_mcp() {
    let mut harness = ArtifactHarness::new();
    harness.generate_spec("cmd-prompt-spec-generate");
    let before = harness.status_state();
    let candidate = before["spec"]["current_candidate"].clone();
    let response = harness.service.handle_codex_user_request(request(
        "planning.spec.approve",
        "cmd-pod-spec-approve",
        Some(&harness.session_id),
        Some(before["revision"].as_u64().unwrap()),
        json!({
            "candidate_id":candidate["candidate_id"],
            "semantic_hash":candidate["semantic_hash"],
            "base_domain_revision":candidate["base_domain_revision"]
        }),
    ));
    assert_eq!(response["ok"], true, "response={response}");
    let store = PlanningStore::open_project(harness.directory.path()).unwrap();
    let event = store
        .event_envelopes(&harness.session_id)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(event.metadata.actor, EventActor::User);
    assert_eq!(event.metadata.adapter, EventAdapter::CodexMcp);
}

fn event_count(harness: &ArtifactHarness) -> u64 {
    PlanningStore::open_project(harness.directory.path())
        .unwrap()
        .event_count(&harness.session_id)
        .unwrap()
}
