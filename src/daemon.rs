use crate::clarification::{self, ClarificationRequest};
use crate::codex;
use crate::config::{self, Config, Paths};
use crate::domain::WorkItem;
use crate::providers;
use crate::storage;
use crate::storage::QueueItem;
use crate::tasks::{self, AgentTask};
use anyhow::{Context, Result, bail};
use std::fs::{self, OpenOptions};
use std::process::{Command, Stdio};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const DAEMON_CHILD_ENV: &str = "SISYPHUS_DAEMON_CHILD";
const REGISTERED_SERVICE_ENV: &str = "SISYPHUS_REGISTERED_SERVICE";

pub async fn serve(daemonize: bool) -> Result<()> {
    if should_spawn_background_child(daemonize) {
        return spawn_background_child();
    }

    run_foreground().await
}

fn should_spawn_background_child(daemonize: bool) -> bool {
    daemonize
        && std::env::var_os(DAEMON_CHILD_ENV).is_none()
        && std::env::var_os(REGISTERED_SERVICE_ENV).is_none()
}

fn spawn_background_child() -> Result<()> {
    let exe = std::env::current_exe().context("failed to resolve current executable")?;
    let paths = Paths::resolve()?;
    paths.ensure_base_dir()?;

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.stdout_log_path)
        .with_context(|| format!("failed to open {}", paths.stdout_log_path.display()))?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.stderr_log_path)
        .with_context(|| format!("failed to open {}", paths.stderr_log_path.display()))?;

    let child = Command::new(exe)
        .arg("serve")
        .env(DAEMON_CHILD_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .context("failed to spawn background daemon")?;

    println!(
        "Sisyphus daemon started in background with pid {}",
        child.id()
    );
    Ok(())
}

async fn run_foreground() -> Result<()> {
    let paths = Paths::resolve()?;
    config::load_or_create(&paths)?;
    storage::initialize(&paths)?;

    remove_stale_socket_or_bail(&paths)?;

    let control = UnixListener::bind(&paths.socket_path)
        .with_context(|| format!("failed to bind {}", paths.socket_path.display()))?;

    println!("Sisyphus backend ready");
    println!("control_socket={}", paths.socket_path.display());

    let mut control_task = tokio::spawn(handle_control(control, paths.clone()));
    let mut polling_task = tokio::spawn(run_polling_loop(paths.clone()));
    let mut dispatch_task = tokio::spawn(run_dispatch_loop(paths.clone()));

    let result = tokio::select! {
        result = shutdown_signal() => {
            result
        }
        result = &mut control_task => {
            result.context("control task failed to join").and_then(|inner| inner)
        }
        result = &mut polling_task => {
            result.context("polling task failed to join").and_then(|inner| inner)
        }
        result = &mut dispatch_task => {
            result.context("dispatch task failed to join").and_then(|inner| inner)
        }
    };

    abort_tasks([control_task, polling_task, dispatch_task]);
    let _ = fs::remove_file(&paths.socket_path);
    result
}

async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut interrupt =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .context("failed to listen for SIGINT")?;
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .context("failed to listen for SIGTERM")?;

        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("failed to listen for ctrl-c")?;
    }

    Ok(())
}

fn abort_tasks(tasks: [JoinHandle<Result<()>>; 3]) {
    for task in tasks {
        task.abort();
    }
}

fn remove_stale_socket_or_bail(paths: &Paths) -> Result<()> {
    if !paths.socket_path.exists() {
        return Ok(());
    }

    if std::os::unix::net::UnixStream::connect(&paths.socket_path).is_ok() {
        bail!(
            "sisyphus daemon already appears to be running at {}",
            paths.socket_path.display()
        );
    }

    fs::remove_file(&paths.socket_path)
        .with_context(|| format!("failed to remove stale {}", paths.socket_path.display()))
}

async fn handle_control(listener: UnixListener, paths: Paths) -> Result<()> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let paths = paths.clone();
        tokio::spawn(async move {
            let mut buf = [0_u8; 2048];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request = if n == 0 {
                "GET /health HTTP/1.1\r\n\r\n".to_string()
            } else {
                String::from_utf8_lossy(&buf[..n]).to_string()
            };
            let response = control_response(&request, &paths).await;
            let _ = stream.write_all(response.as_bytes()).await;
        });
    }
}

async fn run_polling_loop(paths: Paths) -> Result<()> {
    let mut next_delay = Duration::from_secs(Config::default().polling.interval_seconds);

    loop {
        let cfg = match config::load_or_create(&paths) {
            Ok(cfg) => cfg,
            Err(error) => {
                eprintln!("failed to reload config for polling: {error:#}");
                sleep(next_delay).await;
                continue;
            }
        };
        let interval = Duration::from_secs(cfg.polling.interval_seconds);
        let max_backoff = Duration::from_secs(cfg.polling.max_backoff_seconds);

        match poll_once(&paths, &cfg).await {
            Ok(queued) => {
                if queued > 0 {
                    eprintln!("queued {queued} provider work item(s)");
                }
                next_delay = interval;
            }
            Err(error) => {
                eprintln!("provider polling failed: {error:#}");
                next_delay = next_backoff(next_delay, max_backoff);
            }
        }

        sleep(next_delay).await;
    }
}

async fn poll_once(paths: &Paths, cfg: &Config) -> Result<usize> {
    if cfg.providers.is_empty() {
        return Ok(0);
    }

    let items = providers::poll_provider_targets(&cfg.providers).await?;
    enqueue_dispatchable_items(paths, cfg, items)
}

async fn run_dispatch_loop(paths: Paths) -> Result<()> {
    loop {
        let cfg = match config::load_or_create(&paths) {
            Ok(cfg) => cfg,
            Err(error) => {
                eprintln!("failed to reload config for dispatch: {error:#}");
                sleep(Duration::from_secs(
                    Config::default().polling.interval_seconds,
                ))
                .await;
                continue;
            }
        };
        let interval = Duration::from_secs(cfg.polling.interval_seconds);

        if let Err(error) = dispatch_queued_once(&paths, &cfg).await {
            eprintln!("queue dispatch failed: {error:#}");
        }

        sleep(interval).await;
    }
}

async fn dispatch_queued_once(paths: &Paths, cfg: &Config) -> Result<usize> {
    if !cfg.dispatch.auto_dispatch {
        return Ok(0);
    }

    let mut progressed = 0;
    for queue_item in storage::list_queue_items(paths)?
        .into_iter()
        .filter(|queue_item| queue_item.state == "queued")
    {
        if dispatch_queue_item(paths, cfg, queue_item).await? {
            progressed += 1;
        }
    }

    Ok(progressed)
}

async fn dispatch_queue_item(paths: &Paths, cfg: &Config, queue_item: QueueItem) -> Result<bool> {
    let Some((work_item, task)) = dispatch_material_for_queue_item(cfg, &queue_item)? else {
        return Ok(false);
    };

    storage::update_queue_state(paths, queue_item.id, "dispatching")?;
    match codex::dispatch(&task, false) {
        Ok(result) => {
            store_codex_session_ref(paths, queue_item.id, &result)?;
            let next_state = if let Some(request) = &result.clarification_request {
                match publish_clarification_request(cfg, &work_item, queue_item.id, request).await {
                    Ok(()) => "awaiting_clarification",
                    Err(error) => {
                        eprintln!(
                            "failed to publish clarification for queue item {}: {error:#}",
                            queue_item.id
                        );
                        "clarification_publish_failed"
                    }
                }
            } else if result.app_deep_link.is_some() && result.thread_id.is_none() {
                "manual_open_required"
            } else {
                "dispatched"
            };
            storage::update_queue_state(paths, queue_item.id, next_state)?;
            Ok(true)
        }
        Err(error) => {
            storage::update_queue_state(paths, queue_item.id, "dispatch_failed")?;
            eprintln!("failed to dispatch queue item {}: {error:#}", queue_item.id);
            Ok(true)
        }
    }
}

fn store_codex_session_ref(
    paths: &Paths,
    queue_item_id: i64,
    result: &codex::CodexDispatchResult,
) -> Result<()> {
    if result.thread_id.is_none() && result.resume_hint.is_none() && result.app_deep_link.is_none()
    {
        return Ok(());
    }

    storage::upsert_agent_session_ref(
        paths,
        &storage::AgentSessionRef {
            queue_item_id,
            agent: "codex".to_string(),
            dispatch_path: result.path.as_str().to_string(),
            session_id: result.thread_id.clone(),
            resume_hint: result.resume_hint.clone(),
            app_deep_link: result.app_deep_link.clone(),
        },
    )
}

async fn publish_clarification_request(
    cfg: &Config,
    work_item: &WorkItem,
    queue_item_id: i64,
    request: &ClarificationRequest,
) -> Result<()> {
    let target = cfg
        .provider_target_for(work_item)
        .with_context(|| format!("no provider target configured for {}", work_item.source_url))?;
    let run_id = format!("queue-{queue_item_id}");
    let clarification_id = format!("clarification-{queue_item_id}");
    let comment = clarification::format_clarification_comment(&run_id, &clarification_id, request);

    providers::post_issue_comment(target, work_item, &comment).await
}

fn dispatch_material_for_queue_item(
    cfg: &Config,
    queue_item: &QueueItem,
) -> Result<Option<(WorkItem, AgentTask)>> {
    let work_item: WorkItem = serde_json::from_str(&queue_item.payload)
        .with_context(|| format!("failed to parse queue item {} payload", queue_item.id))?;
    let Some(repo_path) = cfg.repository_path_for(&work_item) else {
        return Ok(None);
    };

    let task = tasks::build_codex_task(queue_item, &repo_path)?;
    Ok(Some((work_item, task)))
}

fn enqueue_dispatchable_items(paths: &Paths, cfg: &Config, items: Vec<WorkItem>) -> Result<usize> {
    let mut queued = 0;

    for item in items {
        if cfg.dispatch.should_dispatch(&item.state, &item.labels) {
            storage::enqueue_work_item(paths, &item)?;
            queued += 1;
        }
    }

    Ok(queued)
}

fn next_backoff(current: Duration, max_backoff: Duration) -> Duration {
    let doubled = current.saturating_mul(2);
    if doubled > max_backoff {
        max_backoff
    } else {
        doubled
    }
}

async fn control_response(request: &str, paths: &Paths) -> String {
    let first_line = request.lines().next().unwrap_or_default();

    if first_line.starts_with("GET /health ") {
        return http_response(
            "200 OK",
            "application/json",
            "{\"status\":\"ok\",\"service\":\"sisyphus\"}\n",
        );
    }

    if first_line.starts_with("GET /queue ") {
        return match storage::list_queue_items(paths)
            .and_then(|items| serde_json::to_string(&items).context("failed to serialize queue"))
        {
            Ok(body) => http_response("200 OK", "application/json", &(body + "\n")),
            Err(error) => http_response(
                "500 Internal Server Error",
                "text/plain",
                &format!("{error:#}\n"),
            ),
        };
    }

    if first_line.starts_with("GET /sessions ") {
        return match storage::list_agent_session_refs(paths).and_then(|items| {
            serde_json::to_string(&items).context("failed to serialize session refs")
        }) {
            Ok(body) => http_response("200 OK", "application/json", &(body + "\n")),
            Err(error) => http_response(
                "500 Internal Server Error",
                "text/plain",
                &format!("{error:#}\n"),
            ),
        };
    }

    if first_line.starts_with("GET /events ") {
        return match storage::list_loop_events(paths).and_then(|items| {
            serde_json::to_string(&items).context("failed to serialize loop events")
        }) {
            Ok(body) => http_response("200 OK", "application/json", &(body + "\n")),
            Err(error) => http_response(
                "500 Internal Server Error",
                "text/plain",
                &format!("{error:#}\n"),
            ),
        };
    }

    if let Some(queue_item_id) = parse_dispatch_route(first_line) {
        return match control_dispatch_queue_item(paths, queue_item_id).await {
            Ok(state) => http_response(
                "202 Accepted",
                "application/json",
                &format!("{{\"queue_item_id\":{queue_item_id},\"state\":\"{state}\"}}\n"),
            ),
            Err(error) => {
                let message = format!("{error:#}");
                let status = if message.contains("not found") {
                    "404 Not Found"
                } else if message.contains("not queued")
                    || message.contains("not dispatchable")
                    || message.contains("repository mapping not configured")
                {
                    "409 Conflict"
                } else {
                    "500 Internal Server Error"
                };
                http_response(status, "text/plain", &(message + "\n"))
            }
        };
    }

    http_response("404 Not Found", "text/plain", "not found\n")
}

async fn control_dispatch_queue_item(paths: &Paths, queue_item_id: i64) -> Result<String> {
    let cfg = config::load_or_create(paths)?;
    let queue_item = storage::get_queue_item(paths, queue_item_id)?;

    if !is_manually_dispatchable_state(&queue_item.state) {
        bail!(
            "queue item {queue_item_id} is not dispatchable from state {}",
            queue_item.state
        );
    }

    if dispatch_material_for_queue_item(&cfg, &queue_item)?.is_none() {
        bail!("repository mapping not configured for queue item {queue_item_id}");
    }

    dispatch_queue_item(paths, &cfg, queue_item).await?;
    Ok(storage::get_queue_item(paths, queue_item_id)?.state)
}

fn is_manually_dispatchable_state(state: &str) -> bool {
    matches!(
        state,
        "queued"
            | "awaiting_clarification"
            | "clarification_publish_failed"
            | "dispatch_failed"
            | "manual_open_required"
    )
}

fn parse_dispatch_route(first_line: &str) -> Option<i64> {
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;

    if method != "POST" {
        return None;
    }

    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() != 4
        || !segments[0].is_empty()
        || segments[1] != "queue"
        || segments[3] != "dispatch"
    {
        return None;
    }

    segments[2].parse::<i64>().ok().filter(|id| *id > 0)
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn control_health_response_is_json() {
        let paths = Paths {
            config_path: std::path::PathBuf::from("config.toml"),
            db_path: std::path::PathBuf::from("sisyphus.db"),
            socket_path: std::path::PathBuf::from("sisyphus.sock"),
            stdout_log_path: std::path::PathBuf::from("out.log"),
            stderr_log_path: std::path::PathBuf::from("err.log"),
            base_dir: std::path::PathBuf::from("."),
        };
        let response = control_response("GET /health HTTP/1.1\r\n\r\n", &paths).await;
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"service\":\"sisyphus\""));
    }

    #[tokio::test]
    async fn control_queue_response_returns_items() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-control-queue-test-{}",
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
        storage::initialize(&paths).unwrap();
        let work_item = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        storage::enqueue_work_item(&paths, &work_item).unwrap();

        let response = control_response("GET /queue HTTP/1.1\r\n\r\n", &paths).await;

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("https://github.com/acme/widgets/issues/42"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn control_events_response_returns_loop_events() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-control-events-test-{}",
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
        storage::initialize(&paths).unwrap();
        let work_item = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        storage::enqueue_work_item(&paths, &work_item).unwrap();

        let response = control_response("GET /events HTTP/1.1\r\n\r\n", &paths).await;

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("queue.enqueued"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_control_dispatch_route() {
        assert_eq!(
            parse_dispatch_route("POST /queue/42/dispatch HTTP/1.1"),
            Some(42)
        );
        assert_eq!(
            parse_dispatch_route("GET /queue/42/dispatch HTTP/1.1"),
            None
        );
        assert_eq!(
            parse_dispatch_route("POST /queue/0/dispatch HTTP/1.1"),
            None
        );
        assert_eq!(
            parse_dispatch_route("POST /queue/abc/dispatch HTTP/1.1"),
            None
        );
    }

    #[test]
    fn recognizes_manually_dispatchable_states() {
        assert!(is_manually_dispatchable_state("queued"));
        assert!(is_manually_dispatchable_state("awaiting_clarification"));
        assert!(is_manually_dispatchable_state(
            "clarification_publish_failed"
        ));
        assert!(is_manually_dispatchable_state("dispatch_failed"));
        assert!(is_manually_dispatchable_state("manual_open_required"));
        assert!(!is_manually_dispatchable_state("dispatching"));
        assert!(!is_manually_dispatchable_state("dispatched"));
    }

    #[tokio::test]
    async fn control_dispatch_requires_repository_mapping() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-control-dispatch-test-{}",
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
        let _ = config::load_or_create(&paths).unwrap();
        storage::initialize(&paths).unwrap();
        let work_item = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        let id = storage::enqueue_work_item(&paths, &work_item).unwrap();

        let response = control_response(
            &format!("POST /queue/{id}/dispatch HTTP/1.1\r\n\r\n"),
            &paths,
        )
        .await;

        assert!(response.starts_with("HTTP/1.1 409 Conflict"));
        assert!(response.contains("repository mapping not configured"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn control_dispatch_rejects_non_dispatchable_state() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-control-dispatch-state-test-{}",
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
        let _ = config::load_or_create(&paths).unwrap();
        storage::initialize(&paths).unwrap();
        let work_item = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        let id = storage::enqueue_work_item(&paths, &work_item).unwrap();
        storage::update_queue_state(&paths, id, "dispatched").unwrap();

        let response = control_response(
            &format!("POST /queue/{id}/dispatch HTTP/1.1\r\n\r\n"),
            &paths,
        )
        .await;

        assert!(response.starts_with("HTTP/1.1 409 Conflict"));
        assert!(response.contains("not dispatchable"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn polling_backoff_is_capped() {
        assert_eq!(
            next_backoff(Duration::from_secs(5), Duration::from_secs(60)),
            Duration::from_secs(10)
        );
        assert_eq!(
            next_backoff(Duration::from_secs(40), Duration::from_secs(60)),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn enqueue_dispatchable_items_filters_by_dispatch_config() {
        let root = std::env::temp_dir().join(format!(
            "sisyphus-daemon-dispatch-test-{}",
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
        storage::initialize(&paths).unwrap();

        let mut accepted = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        accepted.labels = vec!["sisyphus".to_string()];

        let mut ignored = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/43").unwrap(),
        );
        ignored.labels = vec!["bug".to_string()];

        let queued =
            enqueue_dispatchable_items(&paths, &Config::default(), vec![accepted, ignored])
                .unwrap();
        let queue = storage::list_queue_items(&paths).unwrap();

        assert_eq!(queued, 1);
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue[0].issue_url,
            "https://github.com/acme/widgets/issues/42"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_material_for_queue_item_requires_registered_repository() {
        let work_item = WorkItem::from_issue_ref(
            crate::providers::parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap(),
        );
        let queue_item = QueueItem {
            id: 1,
            provider: "github".to_string(),
            issue_url: work_item.source_url.clone(),
            state: "queued".to_string(),
            payload: serde_json::to_string(&work_item).unwrap(),
        };

        assert!(
            dispatch_material_for_queue_item(&Config::default(), &queue_item)
                .unwrap()
                .is_none()
        );

        let mut cfg = Config::default();
        cfg.upsert_repository(crate::config::RepositoryConfig {
            kind: crate::domain::Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            path: std::path::PathBuf::from("/repo"),
            instance_url: None,
        });

        let (_, task) = dispatch_material_for_queue_item(&cfg, &queue_item)
            .unwrap()
            .unwrap();
        assert_eq!(task.repo_path, std::path::PathBuf::from("/repo"));
    }
}
