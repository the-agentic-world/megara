use super::planning_store_support::{open_store, start};
use crate::planning::store::{PlanningStore, StoreError};
use rusqlite::Connection;

#[test]
fn sqlite_pragmas_schema_columns_and_plain_scalar_projection_are_fixed() {
    let (_directory, _identity, mut store) = open_store();
    start(&mut store, "cmd_start");
    let connection = &store.conn;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    let synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .unwrap();
    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    let secure_delete: i64 = connection
        .query_row("PRAGMA secure_delete", [], |row| row.get(0))
        .unwrap();
    let busy_timeout: i64 = connection
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");
    assert_eq!(synchronous, 2);
    assert_eq!(foreign_keys, 1);
    assert_eq!(secure_delete, 1);
    assert_eq!(busy_timeout, 5_000);
    let phase: String = connection
        .query_row(
            "SELECT phase FROM sessions WHERE session_id='pln_store'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(phase, "interview");
    for (table, required) in [
        ("project_meta", vec!["key", "value_json"]),
        (
            "sessions",
            vec![
                "session_id",
                "project_id",
                "phase",
                "revision",
                "domain_revision",
                "plan_revision",
                "state_json",
                "normalized_state_hash",
            ],
        ),
        (
            "events",
            vec![
                "event_id",
                "session_id",
                "seq",
                "schema_version",
                "revision_after",
                "domain_revision_after",
                "plan_revision_after",
                "event_type",
                "semantic_payload_json",
                "metadata_json",
                "semantic_payload_hash",
                "state_hash_after",
            ],
        ),
        (
            "command_results",
            vec![
                "command_id",
                "session_id",
                "request_hash",
                "core_response_json",
                "resulting_event_id",
                "resulting_revision",
            ],
        ),
        (
            "purged_sessions",
            vec![
                "session_id",
                "purged_at",
                "purge_schema_version",
                "purge_command_id",
                "request_hash",
                "core_response_json",
                "cleanup_state",
                "pending_backup_id",
            ],
        ),
        ("purged_command_ids", vec!["command_id", "session_id"]),
    ] {
        let columns = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for required_column in required {
            assert!(
                columns.iter().any(|column| column == required_column),
                "{table}.{required_column}"
            );
        }
    }
    let unique_indexes = connection
        .prepare("PRAGMA index_list(events)")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let unique_index = unique_indexes
        .into_iter()
        .find(|(_, unique)| *unique != 0)
        .map(|(name, _)| name)
        .unwrap();
    let unique_columns = connection
        .prepare(&format!("PRAGMA index_info({unique_index})"))
        .unwrap()
        .query_map([], |row| row.get::<_, String>(2))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(unique_columns, ["session_id", "seq"]);
    let foreign_keys = connection
        .prepare("PRAGMA foreign_key_list(events)")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(2)?, row.get::<_, String>(6)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(foreign_keys
        .iter()
        .any(|(table, action)| table == "sessions" && action.eq_ignore_ascii_case("CASCADE")));
}

#[test]
fn only_fresh_empty_schema_zero_is_created_and_version_edges_are_typed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("planning.db");
    let identity = crate::planning::store::ProjectIdentity {
        canonical_root: directory.path().to_string_lossy().into_owned(),
        project_id: "prj_schema_test".to_string(),
    };
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("CREATE TABLE existing_data(value TEXT)", [])
        .unwrap();
    drop(connection);
    assert!(matches!(
        PlanningStore::open(&path, identity.clone()),
        Err(StoreError::SchemaUpgradeRequired {
            actual: 0,
            expected: 1
        })
    ));

    let fresh_path = directory.path().join("fresh.db");
    let fresh = PlanningStore::open(&fresh_path, identity.clone()).unwrap();
    drop(fresh);
    let connection = Connection::open(&fresh_path).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    drop(connection);
    assert!(matches!(
        PlanningStore::open(&fresh_path, identity),
        Err(StoreError::SchemaVersionUnsupported {
            actual: 2,
            expected: 1
        })
    ));

    let corrupt_path = directory.path().join("corrupt.db");
    std::fs::write(&corrupt_path, b"not a sqlite database").unwrap();
    let corrupt_identity = crate::planning::store::ProjectIdentity {
        canonical_root: directory.path().to_string_lossy().into_owned(),
        project_id: "prj_corrupt_test".to_string(),
    };
    assert!(matches!(
        PlanningStore::open(&corrupt_path, corrupt_identity),
        Err(StoreError::DbCorrupt(_))
    ));

    let partial_path = directory.path().join("partial-meta.db");
    let connection = Connection::open(&partial_path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE project_meta(key TEXT PRIMARY KEY, value_json TEXT NOT NULL); INSERT INTO project_meta(key,value_json) VALUES('project_id','\"prj_schema_test\"'); PRAGMA user_version=1;",
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        PlanningStore::open(
            &partial_path,
            crate::planning::store::ProjectIdentity {
                canonical_root: directory.path().to_string_lossy().into_owned(),
                project_id: "prj_schema_test".to_string(),
            }
        ),
        Err(StoreError::DbCorrupt(_))
    ));
}
