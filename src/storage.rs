use crate::config::Paths;
use crate::domain::WorkItem;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i64,
    pub provider: String,
    pub issue_url: String,
    pub state: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionRef {
    pub queue_item_id: i64,
    pub agent: String,
    pub dispatch_path: String,
    pub session_id: Option<String>,
    pub resume_hint: Option<String>,
    pub app_deep_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopEvent {
    pub id: i64,
    pub run_id: Option<String>,
    pub kind: String,
    pub payload: String,
    pub created_at: String,
}

pub fn initialize(paths: &Paths) -> Result<()> {
    paths.ensure_base_dir()?;
    std::fs::create_dir_all(paths.base_dir.join("artifacts")).with_context(|| {
        format!(
            "failed to create {}",
            paths.base_dir.join("artifacts").display()
        )
    })?;

    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS loop_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS work_queue_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider TEXT NOT NULL,
            issue_url TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT 'queued',
            payload TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_work_queue_items_provider_issue_url
        ON work_queue_items(provider, issue_url);

        CREATE TABLE IF NOT EXISTS agent_session_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            queue_item_id INTEGER NOT NULL UNIQUE,
            agent TEXT NOT NULL,
            dispatch_path TEXT NOT NULL,
            session_id TEXT,
            resume_hint TEXT,
            app_deep_link TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(queue_item_id) REFERENCES work_queue_items(id)
        );

        INSERT INTO app_meta(key, value)
        VALUES ('schema_version', '1')
        ON CONFLICT(key) DO NOTHING;
        "#,
    )
    .context("failed to initialize sqlite schema")?;

    Ok(())
}

pub fn enqueue_work_item(paths: &Paths, work_item: &WorkItem) -> Result<i64> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    let payload = serde_json::to_string(work_item).context("failed to serialize work item")?;
    let existing = conn
        .query_row(
            r#"
            SELECT id, state, payload
            FROM work_queue_items
            WHERE provider = ?1 AND issue_url = ?2
            "#,
            params![work_item.provider.as_str(), work_item.source_url.as_str()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .context("failed to read existing work queue item")?;

    conn.execute(
        r#"
        INSERT INTO work_queue_items(provider, issue_url, payload)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(provider, issue_url) DO UPDATE SET
            state = CASE
                WHEN work_queue_items.payload <> excluded.payload
                    AND work_queue_items.state IN (
                        'awaiting_clarification',
                        'clarification_publish_failed',
                        'dispatch_failed',
                        'manual_open_required'
                    )
                THEN 'queued'
                ELSE work_queue_items.state
            END,
            payload = excluded.payload,
            updated_at = CURRENT_TIMESTAMP
        "#,
        params![
            work_item.provider.as_str(),
            work_item.source_url.as_str(),
            payload
        ],
    )
    .context("failed to insert work queue item")?;

    let id = conn
        .query_row(
            r#"
        SELECT id
        FROM work_queue_items
        WHERE provider = ?1 AND issue_url = ?2
        "#,
            params![work_item.provider.as_str(), work_item.source_url.as_str()],
            |row| row.get(0),
        )
        .context("failed to read work queue item id")?;

    match existing {
        None => record_loop_event_conn(
            &conn,
            Some(&queue_run_id(id)),
            "queue.enqueued",
            &json!({
                "queue_item_id": id,
                "state": "queued",
                "provider": work_item.provider.as_str(),
                "issue_url": work_item.source_url,
            }),
        )?,
        Some((_, previous_state, previous_payload))
            if previous_payload != payload && should_requeue_on_payload_change(&previous_state) =>
        {
            record_queue_state_change(&conn, id, &previous_state, "queued")?;
        }
        _ => {}
    }

    Ok(id)
}

pub fn list_queue_items(paths: &Paths) -> Result<Vec<QueueItem>> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, provider, issue_url, state, payload
            FROM work_queue_items
            ORDER BY id ASC
            "#,
        )
        .context("failed to prepare queue query")?;

    let rows = stmt.query_map([], |row| {
        Ok(QueueItem {
            id: row.get(0)?,
            provider: row.get(1)?,
            issue_url: row.get(2)?,
            state: row.get(3)?,
            payload: row.get(4)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read queue items")
}

pub fn get_queue_item(paths: &Paths, id: i64) -> Result<QueueItem> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;

    conn.query_row(
        r#"
        SELECT id, provider, issue_url, state, payload
        FROM work_queue_items
        WHERE id = ?1
        "#,
        params![id],
        |row| {
            Ok(QueueItem {
                id: row.get(0)?,
                provider: row.get(1)?,
                issue_url: row.get(2)?,
                state: row.get(3)?,
                payload: row.get(4)?,
            })
        },
    )
    .with_context(|| format!("failed to read queue item {id}"))
}

pub fn update_queue_state(paths: &Paths, id: i64, state: &str) -> Result<()> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    let previous_state = conn
        .query_row(
            r#"
            SELECT state
            FROM work_queue_items
            WHERE id = ?1
            "#,
            params![id],
            |row| row.get::<_, String>(0),
        )
        .with_context(|| format!("failed to read queue item {id}"))?;

    let changed = conn
        .execute(
            r#"
            UPDATE work_queue_items
            SET state = ?2,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
            params![id, state],
        )
        .with_context(|| format!("failed to update queue item {id}"))?;

    if changed == 0 {
        anyhow::bail!("queue item {id} not found");
    }

    if previous_state != state {
        record_queue_state_change(&conn, id, &previous_state, state)?;
    }

    Ok(())
}

pub fn upsert_agent_session_ref(paths: &Paths, session_ref: &AgentSessionRef) -> Result<()> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;

    conn.execute(
        r#"
        INSERT INTO agent_session_refs(
            queue_item_id,
            agent,
            dispatch_path,
            session_id,
            resume_hint,
            app_deep_link
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(queue_item_id) DO UPDATE SET
            agent = excluded.agent,
            dispatch_path = excluded.dispatch_path,
            session_id = excluded.session_id,
            resume_hint = excluded.resume_hint,
            app_deep_link = excluded.app_deep_link,
            updated_at = CURRENT_TIMESTAMP
        "#,
        params![
            session_ref.queue_item_id,
            session_ref.agent.as_str(),
            session_ref.dispatch_path.as_str(),
            session_ref.session_id.as_deref(),
            session_ref.resume_hint.as_deref(),
            session_ref.app_deep_link.as_deref(),
        ],
    )
    .with_context(|| {
        format!(
            "failed to upsert agent session ref for queue item {}",
            session_ref.queue_item_id
        )
    })?;

    Ok(())
}

pub fn list_agent_session_refs(paths: &Paths) -> Result<Vec<AgentSessionRef>> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT queue_item_id, agent, dispatch_path, session_id, resume_hint, app_deep_link
            FROM agent_session_refs
            ORDER BY queue_item_id ASC
            "#,
        )
        .context("failed to prepare agent session ref query")?;

    let rows = stmt.query_map([], |row| {
        Ok(AgentSessionRef {
            queue_item_id: row.get(0)?,
            agent: row.get(1)?,
            dispatch_path: row.get(2)?,
            session_id: row.get(3)?,
            resume_hint: row.get(4)?,
            app_deep_link: row.get(5)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read agent session refs")
}

pub fn get_agent_session_ref(paths: &Paths, queue_item_id: i64) -> Result<AgentSessionRef> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;

    conn.query_row(
        r#"
        SELECT queue_item_id, agent, dispatch_path, session_id, resume_hint, app_deep_link
        FROM agent_session_refs
        WHERE queue_item_id = ?1
        "#,
        params![queue_item_id],
        |row| {
            Ok(AgentSessionRef {
                queue_item_id: row.get(0)?,
                agent: row.get(1)?,
                dispatch_path: row.get(2)?,
                session_id: row.get(3)?,
                resume_hint: row.get(4)?,
                app_deep_link: row.get(5)?,
            })
        },
    )
    .with_context(|| format!("failed to read agent session ref for queue item {queue_item_id}"))
}

pub fn record_loop_event(
    paths: &Paths,
    run_id: Option<&str>,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    record_loop_event_conn(&conn, run_id, kind, payload)
}

pub fn list_loop_events(paths: &Paths) -> Result<Vec<LoopEvent>> {
    let conn = Connection::open(&paths.db_path)
        .with_context(|| format!("failed to open {}", paths.db_path.display()))?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, run_id, kind, payload, created_at
            FROM loop_events
            ORDER BY id ASC
            "#,
        )
        .context("failed to prepare loop event query")?;

    let rows = stmt.query_map([], |row| {
        Ok(LoopEvent {
            id: row.get(0)?,
            run_id: row.get(1)?,
            kind: row.get(2)?,
            payload: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read loop events")
}

fn record_queue_state_change(
    conn: &Connection,
    queue_item_id: i64,
    from: &str,
    to: &str,
) -> Result<()> {
    record_loop_event_conn(
        conn,
        Some(&queue_run_id(queue_item_id)),
        "queue.state_changed",
        &json!({
            "queue_item_id": queue_item_id,
            "from": from,
            "to": to,
        }),
    )
}

fn record_loop_event_conn(
    conn: &Connection,
    run_id: Option<&str>,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    let payload =
        serde_json::to_string(payload).context("failed to serialize loop event payload")?;
    conn.execute(
        r#"
        INSERT INTO loop_events(run_id, kind, payload)
        VALUES (?1, ?2, ?3)
        "#,
        params![run_id, kind, payload],
    )
    .context("failed to record loop event")?;
    Ok(())
}

fn should_requeue_on_payload_change(state: &str) -> bool {
    matches!(
        state,
        "awaiting_clarification"
            | "clarification_publish_failed"
            | "dispatch_failed"
            | "manual_open_required"
    )
}

fn queue_run_id(queue_item_id: i64) -> String {
    format!("queue-{queue_item_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn initializes_database_file() {
        let root =
            std::env::temp_dir().join(format!("sisyphus-storage-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        assert!(paths.db_path.exists());
        assert!(paths.base_dir.join("artifacts").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enqueues_and_lists_work_item() {
        let root = std::env::temp_dir().join(format!("sisyphus-queue-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let work_item = WorkItem::from_issue_ref(issue_ref);
        let id = enqueue_work_item(&paths, &work_item).unwrap();

        let queue = list_queue_items(&paths).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].id, id);
        assert_eq!(queue[0].provider, "github");
        assert_eq!(
            queue[0].issue_url,
            "https://github.com/acme/widgets/issues/42"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enqueue_deduplicates_by_provider_and_issue_url() {
        let root =
            std::env::temp_dir().join(format!("sisyphus-queue-dedupe-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let mut work_item = WorkItem::from_issue_ref(issue_ref);
        let first_id = enqueue_work_item(&paths, &work_item).unwrap();
        work_item.title = "Updated title".to_string();
        let second_id = enqueue_work_item(&paths, &work_item).unwrap();

        let queue = list_queue_items(&paths).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(first_id, second_id);
        assert!(queue[0].payload.contains("Updated title"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enqueue_keeps_awaiting_clarification_when_payload_is_unchanged() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-queue-clarification-unchanged-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let work_item = WorkItem::from_issue_ref(issue_ref);
        let id = enqueue_work_item(&paths, &work_item).unwrap();
        update_queue_state(&paths, id, "awaiting_clarification").unwrap();

        enqueue_work_item(&paths, &work_item).unwrap();

        let item = get_queue_item(&paths, id).unwrap();
        assert_eq!(item.state, "awaiting_clarification");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enqueue_requeues_awaiting_clarification_when_payload_changes() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-queue-clarification-changed-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let mut work_item = WorkItem::from_issue_ref(issue_ref);
        let id = enqueue_work_item(&paths, &work_item).unwrap();
        update_queue_state(&paths, id, "awaiting_clarification").unwrap();

        work_item.comments.push(crate::domain::IssueComment {
            author: "alice".to_string(),
            body: "Clarification answer added.".to_string(),
            created_at: Some("2026-06-10T00:00:00Z".to_string()),
        });
        enqueue_work_item(&paths, &work_item).unwrap();

        let item = get_queue_item(&paths, id).unwrap();
        assert_eq!(item.state, "queued");
        assert!(item.payload.contains("Clarification answer added."));
        let events = list_loop_events(&paths).unwrap();
        assert!(events.iter().any(|event| {
            event.kind == "queue.state_changed"
                && event.run_id.as_deref() == Some("queue-1")
                && event
                    .payload
                    .contains("\"from\":\"awaiting_clarification\"")
                && event.payload.contains("\"to\":\"queued\"")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gets_and_updates_queue_item_state() {
        let root =
            std::env::temp_dir().join(format!("sisyphus-queue-state-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let work_item = WorkItem::from_issue_ref(issue_ref);
        let id = enqueue_work_item(&paths, &work_item).unwrap();

        update_queue_state(&paths, id, "dispatched").unwrap();
        let item = get_queue_item(&paths, id).unwrap();

        assert_eq!(item.state, "dispatched");
        let events = list_loop_events(&paths).unwrap();
        assert!(events.iter().any(|event| {
            event.kind == "queue.state_changed"
                && event.run_id.as_deref() == Some("queue-1")
                && event.payload.contains("\"from\":\"queued\"")
                && event.payload.contains("\"to\":\"dispatched\"")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upserts_and_lists_agent_session_ref() {
        let root =
            std::env::temp_dir().join(format!("sisyphus-session-ref-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let paths = Paths {
            config_path: root.join("config.toml"),
            db_path: root.join("sisyphus.db"),
            socket_path: root.join("sisyphus.sock"),
            stdout_log_path: root.join("out.log"),
            stderr_log_path: root.join("err.log"),
            base_dir: root.clone(),
        };

        initialize(&paths).unwrap();
        let issue_ref =
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        let work_item = WorkItem::from_issue_ref(issue_ref);
        let queue_item_id = enqueue_work_item(&paths, &work_item).unwrap();

        upsert_agent_session_ref(
            &paths,
            &AgentSessionRef {
                queue_item_id,
                agent: "codex".to_string(),
                dispatch_path: "exec-json".to_string(),
                session_id: Some("thread-1".to_string()),
                resume_hint: Some("codex resume thread-1".to_string()),
                app_deep_link: Some("codex://threads/thread-1".to_string()),
            },
        )
        .unwrap();

        let refs = list_agent_session_refs(&paths).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].queue_item_id, queue_item_id);
        assert_eq!(refs[0].session_id.as_deref(), Some("thread-1"));

        let found = get_agent_session_ref(&paths, queue_item_id).unwrap();
        assert_eq!(found.resume_hint.as_deref(), Some("codex resume thread-1"));

        let _ = fs::remove_dir_all(root);
    }
}
