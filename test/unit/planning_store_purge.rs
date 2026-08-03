use super::planning_store_support::{open_store, start};
use crate::planning::store::{PurgeReceipt, StoreError};
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    Connection,
};

#[test]
fn purge_is_logical_atomic_replayable_and_retires_prior_command_ids() {
    let (_directory, _identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    let receipt = store
        .purge(
            "pln_store",
            "cmd_purge",
            "sha256:purge",
            started.state.revision,
            "pln_store",
        )
        .unwrap();
    let _: PurgeReceipt = receipt.clone();
    assert!(receipt.purged);
    assert!(!receipt.replayed);
    assert!(matches!(
        store.current("pln_store"),
        Err(StoreError::SessionPurged(_))
    ));

    let replayed = store
        .purge(
            "pln_store",
            "cmd_purge",
            "sha256:purge",
            started.state.revision,
            "pln_store",
        )
        .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.cleanup_state, receipt.cleanup_state);
    assert!(matches!(
        store.purge(
            "pln_store",
            "cmd_purge",
            "sha256:other",
            started.state.revision,
            "pln_store",
        ),
        Err(StoreError::CommandIdReuse)
    ));
    assert!(matches!(
        store.purge(
            "pln_store",
            "cmd_other",
            "sha256:other",
            started.state.revision,
            "pln_store",
        ),
        Err(StoreError::SessionPurged(_))
    ));
    assert!(matches!(
        store.purge(
            "pln_store",
            "cmd_start",
            "sha256:request",
            started.state.revision,
            "pln_store",
        ),
        Err(StoreError::CommandIdRetired)
    ));

    let connection = Connection::open(store.database_path()).unwrap();
    let (sessions, events, results, tombstones, retired, purged_at): (
        i64,
        i64,
        i64,
        i64,
        i64,
        String,
    ) = connection
        .query_row(
            "SELECT
                    (SELECT COUNT(*) FROM sessions),
                    (SELECT COUNT(*) FROM events),
                    (SELECT COUNT(*) FROM command_results),
                    (SELECT COUNT(*) FROM purged_sessions),
                    (SELECT COUNT(*) FROM purged_command_ids),
                    (SELECT purged_at FROM purged_sessions WHERE session_id='pln_store')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        (sessions, events, results, tombstones, retired),
        (0, 0, 0, 1, 2)
    );
    assert!(purged_at.contains('.'));
}

#[test]
fn active_command_id_collision_is_reuse_not_retired() {
    let (_directory, _identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    assert!(matches!(
        store.purge(
            "pln_store",
            "cmd_start",
            "sha256:other",
            started.state.revision,
            "pln_store",
        ),
        Err(StoreError::CommandIdReuse)
    ));
}

#[test]
fn cleanup_failure_returns_committed_pending_receipt() {
    let (_directory, _identity, mut store) = open_store();
    let started = start(&mut store, "cmd_start");
    store
        .conn
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Pragma {
                pragma_name: "wal_checkpoint",
                ..
            } => Authorization::Deny,
            _ => Authorization::Allow,
        }));
    let receipt = store
        .purge(
            "pln_store",
            "cmd_purge",
            "sha256:purge",
            started.state.revision,
            "pln_store",
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "pending");
    let cleanup_state: String = store
        .conn
        .query_row(
            "SELECT cleanup_state FROM purged_sessions WHERE session_id='pln_store'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cleanup_state, "pending");
    let replayed = store
        .purge(
            "pln_store",
            "cmd_purge",
            "sha256:purge",
            started.state.revision,
            "pln_store",
        )
        .unwrap();
    assert!(replayed.replayed);
    assert_eq!(replayed.cleanup_state, "pending");
}
