use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;
use crate::planning::store::PlanningStore;
use serde_json::json;
use std::collections::BTreeSet;
use tempfile::tempdir;

#[test]
fn database_byte_scan_excludes_evidence_source_and_claim_text() {
    let directory = tempdir().unwrap();
    let source_marker = "DB_SOURCE_ONLY_MARKER_7b8e";
    let claim_marker = "DB_CLAIM_ONLY_MARKER_3a1f";
    std::fs::write(
        directory.path().join("source.txt"),
        format!("{source_marker}\n"),
    )
    .unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-db-scan-start".to_string(),
        operation: "planning.start".to_string(),
        command_id: Some("cmd-db-scan-start".to_string()),
        session_id: None,
        expected_revision: None,
        params: Some(json!({"request":"database evidence scan"})),
    });
    let session_id = started["session_id"].as_str().unwrap().to_string();
    let response = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-db-scan-evidence".to_string(),
        operation: "planning.evidence.refresh".to_string(),
        command_id: Some("cmd-db-scan-evidence".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(1),
        params: Some(json!({"citations":[{
            "temp_ref":"source", "path":"source.txt", "ranges":[], "claim":claim_marker
        }]})),
    });
    assert_eq!(response["ok"], true, "{response}");
    drop(service);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    let state = store.current(&session_id).unwrap();
    let evidence = state
        .repo_snapshot
        .as_ref()
        .unwrap()
        .evidence
        .first()
        .unwrap();
    let fields = serde_json::to_value(evidence).unwrap();
    assert_eq!(
        fields
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "captured_at",
            "evidence_id",
            "path",
            "ranges",
            "sha256",
            "size",
            "tracked"
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
    );
    let database_path = store.database_path().to_path_buf();
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    for table in ["sessions", "events", "command_results", "project_meta"] {
        let mut statement = connection
            .prepare("SELECT sql FROM sqlite_master WHERE name=?1")
            .unwrap();
        let schema: String = statement.query_row([table], |row| row.get(0)).unwrap();
        assert!(!schema.contains("claim"));
        assert!(!schema.contains("source_text"));
        assert!(!schema.contains("snippet"));
    }
    let cached: Vec<String> = connection
        .prepare("SELECT core_response_json FROM command_results")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(cached
        .iter()
        .all(|value| !value.contains(source_marker) && !value.contains(claim_marker)));
    drop(connection);
    drop(store);
    let mut database_bytes = std::fs::read(&database_path).unwrap();
    let wal_path = database_path.with_extension("db-wal");
    if let Ok(wal) = std::fs::read(wal_path) {
        database_bytes.extend(wal);
    }
    if let Ok(shm) = std::fs::read(database_path.with_extension("db-shm")) {
        database_bytes.extend(shm);
    }
    assert!(!database_bytes
        .windows(source_marker.len())
        .any(|window| window == source_marker.as_bytes()));
    assert!(!database_bytes
        .windows(claim_marker.len())
        .any(|window| window == claim_marker.as_bytes()));
}
