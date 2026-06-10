use anyhow::{Context, Result};
use clap::Parser;
use sisyphus::cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Dashboard) {
        Command::Dashboard => sisyphus::tui::run_dashboard().await,
        Command::Import { issue_url } => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            let issue_ref = sisyphus::providers::parse_issue_url(&issue_url)?;
            let work_item = sisyphus::domain::WorkItem::from_issue_ref(issue_ref);
            let id = sisyphus::storage::enqueue_work_item(&paths, &work_item)?;
            println!("queued work item {id}: {}", work_item.source_url);
            Ok(())
        }
        Command::Queue => {
            let paths = sisyphus::config::Paths::resolve()?;
            sisyphus::storage::initialize(&paths)?;
            for item in sisyphus::storage::list_queue_items(&paths)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    item.id, item.provider, item.state, item.issue_url
                );
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
        Command::Register => sisyphus::register::register_autostart(),
    }
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
