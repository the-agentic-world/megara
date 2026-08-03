use super::planning_store_support::{open_store, snapshot, start};
use crate::planning::engine::CoreError;
use crate::planning::engine::EvidenceRefreshCommand;
use crate::planning::store::{PlanningStore, StoreError};
use rusqlite::Connection;
use std::time::Instant;

fn counts(store: &PlanningStore) -> (i64, i64, i64) {
    let connection = Connection::open(store.database_path()).unwrap();
    connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM sessions),
                (SELECT COUNT(*) FROM events),
                (SELECT COUNT(*) FROM command_results)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

#[test]
fn sqlite_abort_triggers_roll_back_session_event_and_result_boundaries() {
    for (table, trigger) in [
        ("sessions", "abort_sessions"),
        ("events", "abort_events"),
        ("command_results", "abort_results"),
    ] {
        let (_directory, _identity, mut store) = open_store();
        let connection = Connection::open(store.database_path()).unwrap();
        connection
            .execute_batch(&format!(
                "CREATE TRIGGER {trigger} BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;"
            ))
            .unwrap();
        let project_id = store.project_id().to_string();
        assert!(store
            .start(
                "cmd_atomic",
                "sha256:atomic",
                crate::planning::engine::StartCommand {
                    session_id: Some("pln_atomic".to_string()),
                    project_id,
                    request: "atomic boundary".to_string(),
                    title: None,
                },
            )
            .is_err());
        assert_eq!(counts(&store), (0, 0, 0), "failed boundary: {table}");
    }
}

#[test]
fn stale_writer_cannot_overwrite_revision_and_lock_timeout_is_typed_db_busy() {
    let (directory, identity, mut first) = open_store();
    let started = start(&mut first, "cmd_start");
    let database_path = directory.path().join("planning.db");
    let mut second = PlanningStore::open(database_path, identity).unwrap();
    let first_result = first
        .refresh_evidence(
            "cmd_first_refresh",
            "sha256:first",
            EvidenceRefreshCommand {
                session_id: "pln_store".to_string(),
                expected_revision: started.state.revision,
                snapshot: snapshot("sha256:first-evidence"),
            },
        )
        .unwrap();
    assert_eq!(first_result.state.revision, 2);
    let stale = second.refresh_evidence(
        "cmd_stale_refresh",
        "sha256:stale",
        EvidenceRefreshCommand {
            session_id: "pln_store".to_string(),
            expected_revision: started.state.revision,
            snapshot: snapshot("sha256:stale-evidence"),
        },
    );
    assert!(matches!(
        stale,
        Err(StoreError::Core(CoreError::RevisionConflict {
            expected: 1,
            actual: 2
        }))
    ));
    assert_eq!(first.event_count("pln_store").unwrap(), 2);
    assert_eq!(second.current("pln_store").unwrap(), first_result.state);
    assert_eq!(counts(&first), (1, 2, 2));

    let locker = Connection::open(first.database_path()).unwrap();
    locker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let project_id = first.project_id().to_string();
    let started_at = Instant::now();
    let busy = first.start(
        "cmd_busy",
        "sha256:busy",
        crate::planning::engine::StartCommand {
            session_id: Some("pln_busy".to_string()),
            project_id,
            request: "busy".to_string(),
            title: None,
        },
    );
    assert!(matches!(busy, Err(StoreError::DbBusy)));
    assert!(started_at.elapsed().as_millis() >= 4_000);
    locker.execute_batch("ROLLBACK").unwrap();
    assert_eq!(first.event_count("pln_store").unwrap(), 2);
    assert_eq!(counts(&first), (1, 2, 2));
}
