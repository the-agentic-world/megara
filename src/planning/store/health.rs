use std::path::{Component, Path};

use rusqlite::{params, TransactionBehavior};
use serde_json::Value;

use super::super::domain::PlanningState;
use super::persistence;
use super::{PlanningStore, PurgeReceipt, StoreError};

#[derive(Debug)]
pub(crate) struct PlanningHealthIssue {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) repairable: bool,
}

#[derive(Debug)]
pub(crate) struct TombstoneInspection {
    pub(crate) session_id: String,
    pub(crate) cleanup_state: String,
    pub(crate) pending_backup_id: Option<String>,
    pub(crate) artifact_residue: bool,
    pub(crate) backup_residue: bool,
}

#[derive(Debug)]
pub(crate) struct PlanningInspection {
    pub(crate) replayed_states: Vec<PlanningState>,
    pub(crate) cache_repairs: Vec<PlanningState>,
    pub(crate) tombstones: Vec<TombstoneInspection>,
    pub(crate) issues: Vec<PlanningHealthIssue>,
    pub(crate) event_count: u64,
}

impl PlanningStore {
    pub(crate) fn inspect_health(&self) -> Result<PlanningInspection, StoreError> {
        let mut inspection = PlanningInspection {
            replayed_states: Vec::new(),
            cache_repairs: Vec::new(),
            tombstones: Vec::new(),
            issues: Vec::new(),
            event_count: 0,
        };

        let integrity: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            inspection.issues.push(PlanningHealthIssue {
                code: "DB_CORRUPT",
                message: format!("SQLite integrity_check returned {integrity}"),
                repairable: false,
            });
        }

        inspection.event_count = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| {
                row.get::<_, u64>(0)
            })?;

        let session_ids = {
            let mut statement = self
                .conn
                .prepare("SELECT session_id FROM sessions ORDER BY session_id")?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for session_id in session_ids {
            let replayed = match persistence::replay_state_from_events(&self.conn, &session_id) {
                Ok(state) => state,
                Err(error) => {
                    inspection
                        .issues
                        .push(issue_from_error(Some(&session_id), error));
                    continue;
                }
            };
            match persistence::cache_matches_state(&self.conn, &replayed) {
                Ok(true) => {}
                Ok(false) => {
                    inspection.cache_repairs.push(replayed.clone());
                    inspection.issues.push(PlanningHealthIssue {
                        code: "PROJECTION_DIVERGED",
                        message: format!("session cache differs from replay: {session_id}"),
                        repairable: true,
                    });
                }
                Err(error) => {
                    let issue = issue_from_error(Some(&session_id), error);
                    if issue.repairable {
                        inspection.cache_repairs.push(replayed.clone());
                    }
                    inspection.issues.push(issue);
                }
            }
            inspection.replayed_states.push(replayed);
        }

        let tombstones = {
            let mut statement = self.conn.prepare(
                "SELECT session_id, purge_command_id, cleanup_state, pending_backup_id, core_response_json FROM purged_sessions ORDER BY session_id",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (session_id, command_id, cleanup_state, pending_backup_id, response) in tombstones {
            let artifact_residue = artifact_residue(self.project_root(), &session_id);
            let backup_residue = pending_backup_id
                .as_deref()
                .filter(|value| valid_component(value))
                .is_some_and(|backup_id| {
                    self.project_root()
                        .join(".megara/migration-backups")
                        .join(backup_id)
                        .symlink_metadata()
                        .is_ok()
                });
            let tombstone = TombstoneInspection {
                session_id: session_id.clone(),
                cleanup_state: cleanup_state.clone(),
                pending_backup_id: pending_backup_id.clone(),
                artifact_residue,
                backup_residue,
            };
            inspection.tombstones.push(tombstone);

            if !valid_component(&session_id) || !valid_component(&command_id) {
                inspection.issues.push(PlanningHealthIssue {
                    code: "TOMBSTONE_INVALID",
                    message: format!("tombstone identity is not a safe component: {session_id}"),
                    repairable: false,
                });
            }
            if pending_backup_id
                .as_deref()
                .is_some_and(|value| !valid_component(value))
            {
                inspection.issues.push(PlanningHealthIssue {
                    code: "TOMBSTONE_INVALID",
                    message: format!("tombstone backup id is invalid: {session_id}"),
                    repairable: false,
                });
            }
            if !valid_tombstone(&response, &session_id, &cleanup_state) {
                inspection.issues.push(PlanningHealthIssue {
                    code: "TOMBSTONE_INVALID",
                    message: format!("purge tombstone is not minimal or bound: {session_id}"),
                    repairable: false,
                });
            }
            if cleanup_state != "clean" && cleanup_state != "pending" {
                inspection.issues.push(PlanningHealthIssue {
                    code: "TOMBSTONE_INVALID",
                    message: format!(
                        "unknown purge cleanup state for {session_id}: {cleanup_state}"
                    ),
                    repairable: false,
                });
            }
            if cleanup_state == "clean" && pending_backup_id.is_some() {
                inspection.issues.push(PlanningHealthIssue {
                    code: "PURGE_RESIDUE",
                    message: format!("clean tombstone retains pending backup id: {session_id}"),
                    repairable: true,
                });
            }
            if cleanup_state == "pending" || artifact_residue || backup_residue {
                inspection.issues.push(PlanningHealthIssue {
                    code: "PURGE_RESIDUE",
                    message: format!("purge cleanup residue remains: {session_id}"),
                    repairable: true,
                });
            }
        }

        Ok(inspection)
    }

    pub(crate) fn repair_cached_state(&mut self, state: &PlanningState) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE sessions SET project_id=?1, phase=?2, revision=?3, domain_revision=?4, plan_revision=?5, state_json=?6, normalized_state_hash=?7 WHERE session_id=?8",
            params![
                state.project_id,
                persistence::phase_name(state.phase),
                state.revision,
                state.domain_revision,
                state.plan_revision,
                serde_json::to_string(state)?,
                super::normalized_state_hash(state),
                state.session_id,
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::DbCorrupt(format!(
                "cannot repair missing session cache: {}",
                state.session_id
            )));
        }
        tx.commit()?;
        Ok(())
    }
}

fn issue_from_error(session_id: Option<&str>, error: StoreError) -> PlanningHealthIssue {
    let (code, repairable) = match &error {
        StoreError::ProjectionDiverged(_) => ("PROJECTION_DIVERGED", true),
        StoreError::DbCorrupt(_) | StoreError::Json(_) => ("DB_CORRUPT", false),
        _ => ("PLANNING_DOCTOR_ERROR", false),
    };
    let message = match session_id {
        Some(session_id) => format!("session {session_id}: {error}"),
        None => error.to_string(),
    };
    PlanningHealthIssue {
        code,
        message,
        repairable,
    }
}

fn valid_tombstone(response: &str, session_id: &str, cleanup_state: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    let expected = ["cleanup_state", "purged", "replayed", "session_id"];
    if object.keys().any(|key| !expected.contains(&key.as_str()))
        || expected.iter().any(|key| !object.contains_key(*key))
    {
        return false;
    }
    let Ok(receipt) = serde_json::from_value::<PurgeReceipt>(value) else {
        return false;
    };
    receipt.session_id == session_id
        && receipt.purged
        && !receipt.replayed
        && receipt.cleanup_state == cleanup_state
}

fn valid_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn artifact_residue(project_root: &Path, session_id: &str) -> bool {
    if !valid_component(session_id) {
        return false;
    }
    project_root
        .join(".megara/planning/artifacts")
        .join(session_id)
        .symlink_metadata()
        .is_ok()
}
