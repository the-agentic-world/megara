use anyhow::{Context, Result};
use clap::Parser;
use sisyphus::cli::{Cli, Command, QueueCommand};
use sisyphus::domain::WorkItem;
use sisyphus::storage::QueueItem;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Dashboard) {
        Command::Dashboard => sisyphus::tui::run_dashboard().await,
        Command::Import { issue_url } => {
            let paths = sisyphus::config::Paths::resolve()?;
            let cfg = sisyphus::config::load_or_create(&paths)?;
            sisyphus::storage::initialize(&paths)?;
            let issue_ref = sisyphus::providers::parse_issue_url(&issue_url)?;
            let fallback_work_item = sisyphus::domain::WorkItem::from_issue_ref(issue_ref.clone());
            let configured_target = cfg.provider_target_for(&fallback_work_item);
            let fetched = match sisyphus::providers::fetch_issue(&issue_ref, configured_target)
                .await
            {
                Ok(work_item) => Ok(work_item),
                Err(error) if configured_target.is_some() => {
                    eprintln!(
                        "warning: failed to fetch issue details with configured provider auth, retrying without auth: {error:#}"
                    );
                    sisyphus::providers::fetch_issue(&issue_ref, None).await
                }
                Err(error) => Err(error),
            };
            let work_item = fetched.unwrap_or_else(|error| {
                eprintln!(
                    "warning: failed to fetch issue details, queued URL-only item: {error:#}"
                );
                fallback_work_item
            });
            let id = sisyphus::storage::enqueue_work_item(&paths, &work_item)?;
            println!(
                "queued queue_item_id={id} issue=#{} {}",
                work_item.number, work_item.source_url
            );
            Ok(())
        }
        Command::Queue { command } => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            match command {
                None => {
                    for item in sisyphus::storage::list_queue_items(&paths)? {
                        print_queue_item_row(&item);
                    }
                }
                Some(QueueCommand::Show { queue_item_id }) => {
                    let item = sisyphus::storage::get_queue_item(&paths, queue_item_id)?;
                    print_queue_item_detail(&item)?;
                }
                Some(QueueCommand::Retry { queue_item_id }) => {
                    let item = sisyphus::storage::retry_queue_item(&paths, queue_item_id)?;
                    println!("requeued work item {}: {}", item.id, item.issue_url);
                }
                Some(QueueCommand::Pause { queue_item_id }) => {
                    let item = sisyphus::storage::pause_queue_item(&paths, queue_item_id)?;
                    println!("paused work item {}: {}", item.id, item.issue_url);
                }
                Some(QueueCommand::Resume { queue_item_id }) => {
                    let item = sisyphus::storage::resume_queue_item(&paths, queue_item_id)?;
                    println!("resumed work item {}: {}", item.id, item.issue_url);
                }
                Some(QueueCommand::Cancel { queue_item_id }) => {
                    let item = sisyphus::storage::cancel_queue_item(&paths, queue_item_id)?;
                    println!("canceled work item {}: {}", item.id, item.issue_url);
                }
                Some(QueueCommand::Remove { queue_item_id }) => {
                    sisyphus::storage::remove_queue_item(&paths, queue_item_id)?;
                    println!("removed work item {queue_item_id}");
                }
            }
            Ok(())
        }
        Command::Sessions => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            for session_ref in sisyphus::storage::list_agent_session_refs(&paths)? {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    session_ref.queue_item_id,
                    session_ref.agent,
                    session_ref.dispatch_path,
                    session_ref.resume_hint.unwrap_or_default(),
                    session_ref.app_deep_link.unwrap_or_default()
                );
            }
            Ok(())
        }
        Command::Events => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            for event in sisyphus::storage::list_loop_events(&paths)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    event.id,
                    event.run_id.unwrap_or_default(),
                    event.kind,
                    event.payload
                );
            }
            Ok(())
        }
        Command::Open { queue_item_id } => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            let session_ref = sisyphus::storage::get_agent_session_ref(&paths, queue_item_id)?;

            println!("queue_item_id={}", session_ref.queue_item_id);
            println!("agent={}", session_ref.agent);
            println!("dispatch_path={}", session_ref.dispatch_path);
            if let Some(resume_hint) = &session_ref.resume_hint {
                println!("codex_cli_resume={resume_hint}");
            }
            if let Some(app_deep_link) = &session_ref.app_deep_link {
                println!("codex_app_deep_link={app_deep_link}");
                open_app_deep_link(app_deep_link)?;
            }
            Ok(())
        }
        Command::Retry { queue_item_id } => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            let item = sisyphus::storage::retry_queue_item(&paths, queue_item_id)?;
            println!(
                "requeued work item {}: {} {}",
                item.id, item.provider, item.issue_url
            );
            Ok(())
        }
        Command::Dispatch {
            queue_item_id,
            dry_run,
            repo_path,
        } => {
            let paths = sisyphus::config::Paths::resolve()?;
            let cfg = sisyphus::config::load_or_create(&paths)?;
            sisyphus::storage::initialize(&paths)?;
            let queue_item = sisyphus::storage::get_queue_item(&paths, queue_item_id)?;
            let work_item: sisyphus::domain::WorkItem = serde_json::from_str(&queue_item.payload)
                .with_context(|| {
                format!("failed to parse queue item {queue_item_id} payload")
            })?;
            let repo_path = repo_path
                .or_else(|| cfg.repository_path_for(&work_item))
                .unwrap_or(std::env::current_dir()?);
            let task = sisyphus::tasks::build_codex_task(&queue_item, &repo_path)?;
            let result = sisyphus::codex::dispatch(&task, dry_run)?;

            println!("codex_dispatch_path={}", result.path.as_str());
            if dry_run {
                println!("{}", task.prompt);
                return Ok(());
            }

            if result.thread_id.is_some()
                || result.resume_hint.is_some()
                || result.app_deep_link.is_some()
            {
                sisyphus::storage::upsert_agent_session_ref(
                    &paths,
                    &sisyphus::storage::AgentSessionRef {
                        queue_item_id,
                        agent: "codex".to_string(),
                        dispatch_path: result.path.as_str().to_string(),
                        session_id: result.thread_id.clone(),
                        resume_hint: result.resume_hint.clone(),
                        app_deep_link: result.app_deep_link.clone(),
                    },
                )?;
            }

            if result.thread_id.is_some() {
                sisyphus::storage::update_queue_state(&paths, queue_item_id, "dispatched")?;
            } else if result.app_deep_link.is_some() {
                sisyphus::storage::update_queue_state(
                    &paths,
                    queue_item_id,
                    "manual_open_required",
                )?;
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        Command::CodexProbe => {
            let capabilities = sisyphus::codex::CodexCapabilities::probe();
            println!("{}", serde_json::to_string_pretty(&capabilities)?);
            println!(
                "preferred_dispatch_path={}",
                capabilities.preferred_dispatch_path().as_str()
            );
            Ok(())
        }
        Command::Auth {
            provider,
            client_id,
            scopes,
        } => {
            let kind = provider
                .parse::<sisyphus::domain::Provider>()
                .map_err(|error| anyhow::anyhow!("failed to parse provider for auth: {error}"))?;
            let token = if kind == sisyphus::domain::Provider::GitHub {
                let client_id = sisyphus::auth::resolve_github_oauth_client_id(client_id);
                sisyphus::auth::authenticate_github_device_flow(&client_id, &scopes).await?
            } else {
                sisyphus::auth::prompt_for_provider_token(&kind)?
            };
            sisyphus::auth::store_provider_token(&kind, &token)?;
            println!("{} token stored in the OS credential store", kind.as_str());
            Ok(())
        }
        Command::ProviderAdd {
            provider,
            owner_or_namespace,
            repo,
            token_env,
            instance_url,
        } => {
            let paths = sisyphus::config::Paths::resolve()?;
            let mut cfg = sisyphus::config::load_or_create(&paths)?;
            let kind = provider
                .parse::<sisyphus::domain::Provider>()
                .map_err(|error| {
                    anyhow::anyhow!("failed to parse provider for provider-add: {error}")
                })?;
            cfg.upsert_provider_target(sisyphus::config::ProviderTargetConfig {
                kind,
                owner_or_namespace,
                repo,
                instance_url,
                token_env,
            });
            sisyphus::config::save(&paths, &cfg)?;
            println!(
                "provider polling target registered in {}",
                paths.config_path.display()
            );
            Ok(())
        }
        Command::RepoAdd {
            provider,
            owner_or_namespace,
            repo,
            path,
            instance_url,
        } => {
            let paths = sisyphus::config::Paths::resolve()?;
            let mut cfg = sisyphus::config::load_or_create(&paths)?;
            let kind = provider
                .parse::<sisyphus::domain::Provider>()
                .map_err(|error| {
                    anyhow::anyhow!("failed to parse provider for repo-add: {error}")
                })?;
            cfg.upsert_repository(sisyphus::config::RepositoryConfig {
                kind,
                owner_or_namespace,
                repo,
                path,
                instance_url,
            });
            sisyphus::config::save(&paths, &cfg)?;
            println!("repository registered in {}", paths.config_path.display());
            Ok(())
        }
        Command::Serve { daemon } => sisyphus::daemon::serve(daemon).await,
        Command::Stop => {
            let paths = sisyphus::config::Paths::resolve()?;
            let body = control_post(&paths, "/shutdown")?;
            println!("{body}");
            Ok(())
        }
        Command::Register => sisyphus::register::register_autostart(),
    }
}

fn print_queue_item_row(item: &QueueItem) {
    println!(
        "{}\t{}\t{}\t{}\t{}",
        item.id,
        item.provider,
        issue_number_label(item),
        item.state,
        item.issue_url
    );
}

fn print_queue_item_detail(item: &QueueItem) -> Result<()> {
    println!("id={}", item.id);
    println!("provider={}", item.provider);
    println!("issue={}", issue_number_label(item));
    println!("state={}", item.state);
    println!("issue_url={}", item.issue_url);
    let payload: serde_json::Value =
        serde_json::from_str(&item.payload).context("failed to parse queue item payload")?;
    println!("payload={}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn issue_number_label(item: &QueueItem) -> String {
    serde_json::from_str::<WorkItem>(&item.payload)
        .map(|work_item| format!("#{}", work_item.number))
        .unwrap_or_else(|_| "-".to_string())
}

fn control_post(paths: &sisyphus::config::Paths, route: &str) -> Result<String> {
    let mut stream = UnixStream::connect(&paths.socket_path)
        .with_context(|| format!("failed to connect to {}", paths.socket_path.display()))?;
    stream.set_read_timeout(Some(CONTROL_TIMEOUT))?;
    stream.set_write_timeout(Some(CONTROL_TIMEOUT))?;
    let request = format!("POST {route} HTTP/1.1\r\nhost: sisyphus\r\ncontent-length: 0\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    let _ = stream.shutdown(std::net::Shutdown::Write);

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    successful_response_body(&response).map(|body| body.trim().to_string())
}

fn successful_response_body(response: &str) -> Result<&str> {
    let status_line = response
        .lines()
        .next()
        .context("missing HTTP status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .context("missing HTTP status code")?
        .parse::<u16>()
        .context("invalid HTTP status code")?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .context("missing HTTP response body")?;

    if !(200..300).contains(&status) {
        anyhow::bail!("{}", body.trim());
    }

    Ok(body)
}

fn open_app_deep_link(app_deep_link: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(app_deep_link)
            .status()
            .context("failed to execute macOS open")?;
        if !status.success() {
            anyhow::bail!("macOS open failed for {app_deep_link}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("open this URL with Codex App: {app_deep_link}");
    }

    Ok(())
}
