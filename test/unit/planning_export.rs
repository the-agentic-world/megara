use std::fs;

use crate::planning::store::PlanningStore;
use crate::planning_artifact_support::{assert_ok, request, ArtifactHarness};
use serde_json::json;

fn export_request(
    harness: &ArtifactHarness,
    command_id: &str,
    output: &std::path::Path,
    format: &str,
    include_transcript: bool,
    force: bool,
) -> crate::planning::protocol::LogicalRequest {
    request(
        "planning.export",
        command_id,
        Some(&harness.session_id),
        None,
        json!({
            "out":output,
            "format":format,
            "include_transcript":include_transcript,
            "force":force
        }),
    )
}

#[test]
fn approved_bundle_has_exact_default_tree_and_manifest_bindings() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    let output = harness.directory.path().join("bundle");
    let response = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-default",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_ok(&response);
    let mut files = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["manifest.json", "plan.md", "spec.md"]);
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("manifest.json")).unwrap()).unwrap();
    let state = harness.status_state();
    for kind in ["spec", "plan"] {
        let candidate = state[kind]["current_candidate"].clone();
        assert_eq!(manifest[kind]["candidate_id"], candidate["candidate_id"]);
        assert_eq!(manifest[kind]["semantic_hash"], candidate["semantic_hash"]);
    }
    assert_eq!(manifest["include_transcript"], false);
    assert_eq!(manifest["transcript_included"], false);
    assert_eq!(manifest["events_included"], false);
    assert!(!files.iter().any(|file| file == "events.jsonl"));
    assert!(!files.iter().any(|file| file == "transcript.json"));
    assert_eq!(
        response["result"]["path"],
        output.to_string_lossy().to_string()
    );
    assert_eq!(response["result"]["format"], "bundle");
}

#[test]
fn include_transcript_is_opt_in_and_default_bundle_has_no_raw_payload_files() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    let default_output = harness.directory.path().join("default-bundle");
    let default_response = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-private-default",
        &default_output,
        "bundle",
        false,
        false,
    ));
    assert_ok(&default_response);
    assert!(!default_output.join("transcript.json").exists());
    assert!(!default_output.join("events.jsonl").exists());

    let transcript_output = harness.directory.path().join("transcript-bundle");
    let transcript_response = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-transcript",
        &transcript_output,
        "bundle",
        true,
        false,
    ));
    assert_ok(&transcript_response);
    assert!(transcript_output.join("transcript.json").is_file());
    assert!(transcript_output.join("events.jsonl").is_file());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(transcript_output.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["include_transcript"], true);
    assert_eq!(manifest["transcript_included"], true);
    assert_eq!(manifest["events_included"], true);
}

#[test]
fn default_bundle_excludes_secret_marker_and_opt_in_transcript_contains_it() {
    let marker = "EXPORT_SECRET_MARKER_7F3A";
    let mut harness = ArtifactHarness::with_initial_request(marker);
    harness.complete();
    let default_output = harness.directory.path().join("marker-default");
    let default_response = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-marker-default",
        &default_output,
        "bundle",
        false,
        false,
    ));
    assert_ok(&default_response);
    for entry in fs::read_dir(&default_output).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            assert!(!fs::read(entry.path())
                .unwrap()
                .windows(marker.len())
                .any(|bytes| bytes == marker.as_bytes()));
        }
    }

    let transcript_output = harness.directory.path().join("marker-transcript");
    let transcript_response = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-marker-transcript",
        &transcript_output,
        "bundle",
        true,
        false,
    ));
    assert_ok(&transcript_response);
    let transcript = fs::read(transcript_output.join("transcript.json")).unwrap();
    let events = fs::read(transcript_output.join("events.jsonl")).unwrap();
    assert!(transcript
        .windows(marker.len())
        .any(|bytes| bytes == marker.as_bytes()));
    assert!(events
        .windows(marker.len())
        .any(|bytes| bytes == marker.as_bytes()));
}

#[test]
fn bundle_stale_and_missing_approval_are_blocked_before_filesystem_write() {
    let mut harness = ArtifactHarness::new();
    let output = harness.directory.path().join("stale-bundle");
    let state = harness.status_state();
    let missing = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-missing-approval",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_eq!(missing["error"]["code"], "INVALID_PHASE");
    assert!(!output.exists());
    assert_eq!(harness.status_state()["revision"], state["revision"]);

    harness.complete();
    fs::write(
        harness.directory.path().join("src/main.rs"),
        "fn main() { changed(); }\n",
    )
    .unwrap();
    let before = harness.status_state();
    let event_count = PlanningStore::open_project(harness.directory.path())
        .unwrap()
        .event_count(&harness.session_id)
        .unwrap();
    let stale = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-stale",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_eq!(stale["error"]["code"], "EVIDENCE_STALE");
    assert!(!output.exists());
    assert_eq!(harness.status_state()["revision"], before["revision"]);
    assert_eq!(
        PlanningStore::open_project(harness.directory.path())
            .unwrap()
            .event_count(&harness.session_id)
            .unwrap(),
        event_count
    );
}

#[test]
fn recovery_formats_allow_stale_state_and_event_exports() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    fs::write(
        harness.directory.path().join("src/main.rs"),
        "fn main() { stale(); }\n",
    )
    .unwrap();
    for (format, suffix) in [
        ("state-json", "state.json"),
        ("events-jsonl", "events.jsonl"),
    ] {
        let output = harness.directory.path().join(suffix);
        let response = harness.service.handle_user_request(export_request(
            &harness,
            &format!("cmd-export-{format}"),
            &output,
            format,
            false,
            false,
        ));
        assert_ok(&response);
        assert!(output.is_file());
        assert_eq!(response["result"]["format"], format);
    }
}

#[test]
fn export_is_no_event_idempotent_and_rejects_changed_request_hash() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    let output = harness.directory.path().join("replay-bundle");
    let first = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-replay",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_ok(&first);
    let store = PlanningStore::open_project(harness.directory.path()).unwrap();
    let event_count = store.event_count(&harness.session_id).unwrap();
    let revision = first["revision"].as_u64().unwrap();
    let replay = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-replay",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_ok(&replay);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["revision"], revision);
    let store = PlanningStore::open_project(harness.directory.path()).unwrap();
    assert_eq!(store.event_count(&harness.session_id).unwrap(), event_count);

    let changed_output = harness.directory.path().join("changed-output");
    let reused = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-replay",
        &changed_output,
        "bundle",
        false,
        false,
    ));
    assert_eq!(reused["error"]["code"], "COMMAND_ID_REUSE");
    assert!(!changed_output.exists());
}

#[test]
fn existing_output_is_preserved_without_force_and_replaced_with_force() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    let output = harness.directory.path().join("sentinel-output");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("sentinel.txt"), "DO_NOT_OVERWRITE").unwrap();
    let before = harness.status_state();
    let blocked = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-sentinel-blocked",
        &output,
        "bundle",
        false,
        false,
    ));
    assert_eq!(blocked["error"]["code"], "PROJECTION_DIVERGED");
    assert_eq!(
        fs::read_to_string(output.join("sentinel.txt")).unwrap(),
        "DO_NOT_OVERWRITE"
    );
    assert_eq!(harness.status_state()["revision"], before["revision"]);

    let replaced = harness.service.handle_user_request(export_request(
        &harness,
        "cmd-export-sentinel-force",
        &output,
        "bundle",
        false,
        true,
    ));
    assert_ok(&replaced);
    let mut files = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["manifest.json", "plan.md", "spec.md"]);
    assert!(!output.join("sentinel.txt").exists());
}

#[test]
fn logical_export_requires_format_even_though_cli_supplies_bundle() {
    let mut harness = ArtifactHarness::new();
    harness.complete();
    let output = harness.directory.path().join("missing-format");
    let response = harness.service.handle_user_request(request(
        "planning.export",
        "cmd-export-missing-format",
        Some(&harness.session_id),
        None,
        json!({"out":output}),
    ));
    assert_eq!(response["error"]["code"], "INVALID_REQUEST");
    assert!(!output.exists());
}
