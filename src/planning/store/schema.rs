use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, OpenFlags, OptionalExtension};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::*;

pub fn canonical_project_identity(root: &Path) -> Result<ProjectIdentity, StoreError> {
    let canonical = fs::canonicalize(root)?;
    let text = canonical
        .to_str()
        .ok_or_else(|| StoreError::ProjectIdentity("project root is not UTF-8".to_string()))?
        .nfc()
        .collect::<String>()
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    Ok(ProjectIdentity {
        canonical_root: text,
        project_id: format!("prj_{:x}", hasher.finalize()),
    })
}

pub fn open_project(root: impl AsRef<Path>) -> Result<PlanningStore, StoreError> {
    let identity = canonical_project_identity(root.as_ref())?;
    let database_path = PathBuf::from(&identity.canonical_root).join(PLANNING_DB_RELATIVE_PATH);
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent)?;
    }
    open(database_path, identity)
}

pub fn open_existing_project(root: impl AsRef<Path>) -> Result<Option<PlanningStore>, StoreError> {
    let identity = canonical_project_identity(root.as_ref())?;
    let database_path = PathBuf::from(&identity.canonical_root).join(PLANNING_DB_RELATIVE_PATH);
    let metadata = match fs::symlink_metadata(&database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() {
        return Err(StoreError::DbCorrupt(
            "planning database is not a regular file".to_string(),
        ));
    }
    let conn =
        rusqlite::Connection::open_with_flags(&database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(Duration::from_millis(5_000))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != STORE_SCHEMA_VERSION {
        return Err(if version > STORE_SCHEMA_VERSION {
            StoreError::SchemaVersionUnsupported {
                actual: version,
                expected: STORE_SCHEMA_VERSION,
            }
        } else {
            StoreError::SchemaUpgradeRequired {
                actual: version,
                expected: STORE_SCHEMA_VERSION,
            }
        });
    }
    ensure_identity_read_only(&conn, &identity)?;
    Ok(Some(PlanningStore {
        conn,
        identity,
        database_path,
    }))
}

pub fn open(
    database_path: impl AsRef<Path>,
    identity: ProjectIdentity,
) -> Result<PlanningStore, StoreError> {
    let database_path = database_path.as_ref().to_path_buf();
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut conn = rusqlite::Connection::open(&database_path)?;
    configure(&conn)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > STORE_SCHEMA_VERSION {
        return Err(StoreError::SchemaVersionUnsupported {
            actual: version,
            expected: STORE_SCHEMA_VERSION,
        });
    }
    if version < STORE_SCHEMA_VERSION {
        if version != 0 {
            return Err(StoreError::SchemaUpgradeRequired {
                actual: version,
                expected: STORE_SCHEMA_VERSION,
            });
        }
        if has_existing_objects(&conn)? {
            return Err(StoreError::SchemaUpgradeRequired {
                actual: version,
                expected: STORE_SCHEMA_VERSION,
            });
        }
        create(&conn)?;
        conn.pragma_update(None, "user_version", STORE_SCHEMA_VERSION)?;
    }
    ensure_identity(&mut conn, &identity)?;
    Ok(PlanningStore {
        conn,
        identity,
        database_path,
    })
}

fn has_existing_objects(conn: &rusqlite::Connection) -> Result<bool, StoreError> {
    let exists: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('table','index','trigger','view') AND name NOT LIKE 'sqlite_%')",
        [],
        |row| row.get(0),
    )?;
    Ok(exists != 0)
}

fn configure(conn: &rusqlite::Connection) -> Result<(), StoreError> {
    conn.busy_timeout(Duration::from_millis(5_000))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=FULL;
         PRAGMA foreign_keys=ON;
         PRAGMA secure_delete=ON;",
    )?;
    Ok(())
}

fn create(conn: &rusqlite::Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_meta(
            key TEXT PRIMARY KEY NOT NULL,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions(
            session_id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            phase TEXT NOT NULL,
            revision INTEGER NOT NULL,
            domain_revision INTEGER NOT NULL,
            plan_revision INTEGER NOT NULL,
            state_json TEXT NOT NULL,
            normalized_state_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS events(
            event_id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL,
            seq INTEGER NOT NULL,
            schema_version INTEGER NOT NULL,
            revision_after INTEGER NOT NULL,
            domain_revision_after INTEGER NOT NULL,
            plan_revision_after INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            semantic_payload_json TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            semantic_payload_hash TEXT NOT NULL,
            state_hash_after TEXT NOT NULL,
            UNIQUE(session_id, seq),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS command_results(
            command_id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            core_response_json TEXT NOT NULL,
            resulting_event_id TEXT,
            resulting_revision INTEGER NOT NULL,
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS purged_sessions(
            session_id TEXT PRIMARY KEY NOT NULL,
            purged_at TEXT NOT NULL,
            purge_schema_version INTEGER NOT NULL,
            purge_command_id TEXT UNIQUE NOT NULL,
            request_hash TEXT NOT NULL,
            core_response_json TEXT NOT NULL,
            cleanup_state TEXT NOT NULL,
            pending_backup_id TEXT
        );
        CREATE TABLE IF NOT EXISTS purged_command_ids(
            command_id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn ensure_identity(
    conn: &mut rusqlite::Connection,
    identity: &ProjectIdentity,
) -> Result<(), StoreError> {
    let stored_project: Option<String> = conn
        .query_row(
            "SELECT value_json FROM project_meta WHERE key='project_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let stored_root: Option<String> = conn
        .query_row(
            "SELECT value_json FROM project_meta WHERE key='canonical_root'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match (stored_project, stored_root) {
        (Some(project_value), Some(root_value)) => {
            let actual: String = serde_json::from_str(&project_value)
                .map_err(|_| StoreError::DbCorrupt("project_id metadata is invalid".to_string()))?;
            let actual_root: String = serde_json::from_str(&root_value).map_err(|_| {
                StoreError::DbCorrupt("canonical_root metadata is invalid".to_string())
            })?;
            if actual != identity.project_id {
                return Err(StoreError::ProjectIdMismatch {
                    expected: identity.project_id.clone(),
                    actual,
                });
            }
            if actual_root != identity.canonical_root {
                return Err(StoreError::ProjectIdentity(
                    "canonical_root metadata mismatch".to_string(),
                ));
            }
            Ok(())
        }
        (None, None) => {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO project_meta(key, value_json) VALUES('project_id', ?1)",
                params![serde_json::to_string(&identity.project_id)?],
            )?;
            tx.execute(
                "INSERT INTO project_meta(key, value_json) VALUES('canonical_root', ?1)",
                params![serde_json::to_string(&identity.canonical_root)?],
            )?;
            tx.commit()?;
            Ok(())
        }
        _ => Err(StoreError::DbCorrupt(
            "project metadata must contain project_id and canonical_root together".to_string(),
        )),
    }
}

fn ensure_identity_read_only(
    conn: &rusqlite::Connection,
    identity: &ProjectIdentity,
) -> Result<(), StoreError> {
    let stored_project: Option<String> = conn
        .query_row(
            "SELECT value_json FROM project_meta WHERE key='project_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let stored_root: Option<String> = conn
        .query_row(
            "SELECT value_json FROM project_meta WHERE key='canonical_root'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match (stored_project, stored_root) {
        (Some(project_value), Some(root_value)) => {
            let actual: String = serde_json::from_str(&project_value)
                .map_err(|_| StoreError::DbCorrupt("project_id metadata is invalid".to_string()))?;
            let actual_root: String = serde_json::from_str(&root_value).map_err(|_| {
                StoreError::DbCorrupt("canonical_root metadata is invalid".to_string())
            })?;
            if actual != identity.project_id {
                return Err(StoreError::ProjectIdMismatch {
                    expected: identity.project_id.clone(),
                    actual,
                });
            }
            if actual_root != identity.canonical_root {
                return Err(StoreError::ProjectIdentity(
                    "canonical_root metadata mismatch".to_string(),
                ));
            }
            Ok(())
        }
        _ => Err(StoreError::DbCorrupt(
            "planning database identity metadata is incomplete".to_string(),
        )),
    }
}
