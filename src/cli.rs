use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "sisyphus")]
#[command(about = "Local issue-to-agent broker and lifecycle controller")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    #[command(hide = true)]
    Dashboard,
    #[command(about = "Import an issue URL into the local queue")]
    Import { issue_url: String },
    #[command(about = "List and manage local queued work items")]
    Queue {
        #[command(subcommand)]
        command: Option<QueueCommand>,
    },
    #[command(about = "List agent session references known to Sisyphus")]
    Sessions,
    #[command(about = "List local lifecycle events")]
    Events,
    #[command(about = "Open or print a Codex-native session reference")]
    Open { queue_item_id: i64 },
    #[command(about = "Move a stuck or failed work item back to queued")]
    Retry { queue_item_id: i64 },
    #[command(about = "Dispatch a queued work item to Codex")]
    Dispatch {
        queue_item_id: i64,
        #[arg(long, help = "Print the generated task without starting Codex")]
        dry_run: bool,
        #[arg(long, help = "Local repository path Codex should use")]
        repo_path: Option<PathBuf>,
    },
    #[command(about = "Probe local Codex integration capabilities")]
    CodexProbe,
    #[command(about = "Store a provider token in the OS credential store")]
    Auth {
        provider: String,
        #[arg(long, help = "GitHub OAuth App client ID; required for GitHub auth")]
        client_id: Option<String>,
        #[arg(
            long = "scope",
            help = "GitHub OAuth scope for device login; defaults to repo"
        )]
        scopes: Vec<String>,
    },
    #[command(about = "Register a provider repository polling target")]
    ProviderAdd {
        provider: String,
        owner_or_namespace: String,
        repo: String,
        #[arg(long, help = "Environment variable containing the provider token")]
        token_env: Option<String>,
        #[arg(
            long,
            help = "Provider instance URL; defaults to github.com or gitlab.com"
        )]
        instance_url: Option<String>,
    },
    #[command(about = "Register a provider repository to a local workspace path")]
    RepoAdd {
        provider: String,
        owner_or_namespace: String,
        repo: String,
        path: PathBuf,
        #[arg(
            long,
            help = "Provider instance URL; defaults to github.com or gitlab.com"
        )]
        instance_url: Option<String>,
    },
    #[command(about = "Run the local Sisyphus backend")]
    Serve {
        #[arg(
            long,
            alias = "demon",
            help = "Run the backend headlessly in the background"
        )]
        daemon: bool,
    },
    #[command(about = "Stop the running local Sisyphus backend")]
    Stop,
    #[command(
        alias = "resgister",
        about = "Register reboot-persistent autostart for `sisyphus serve --daemon`"
    )]
    Register,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum QueueCommand {
    #[command(about = "Show queue item details")]
    Show { queue_item_id: i64 },
    #[command(about = "Move a stuck or failed work item back to queued")]
    Retry { queue_item_id: i64 },
    #[command(about = "Pause a queued work item")]
    Pause { queue_item_id: i64 },
    #[command(about = "Resume a paused work item")]
    Resume { queue_item_id: i64 },
    #[command(about = "Cancel a work item")]
    Cancel { queue_item_id: i64 },
    #[command(about = "Remove a work item from the local queue")]
    Remove { queue_item_id: i64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_serve_daemon() {
        let cli = Cli::parse_from(["sisyphus", "serve", "--daemon"]);
        assert_eq!(cli.command, Some(Command::Serve { daemon: true }));
    }

    #[test]
    fn parses_serve_demon_alias() {
        let cli = Cli::parse_from(["sisyphus", "serve", "--demon"]);
        assert_eq!(cli.command, Some(Command::Serve { daemon: true }));
    }

    #[test]
    fn parses_register() {
        let cli = Cli::parse_from(["sisyphus", "register"]);
        assert_eq!(cli.command, Some(Command::Register));
    }

    #[test]
    fn parses_stop() {
        let cli = Cli::parse_from(["sisyphus", "stop"]);
        assert_eq!(cli.command, Some(Command::Stop));
    }

    #[test]
    fn parses_resgister_alias() {
        let cli = Cli::parse_from(["sisyphus", "resgister"]);
        assert_eq!(cli.command, Some(Command::Register));
    }

    #[test]
    fn parses_import() {
        let cli = Cli::parse_from(["sisyphus", "import", "https://github.com/o/r/issues/1"]);
        assert_eq!(
            cli.command,
            Some(Command::Import {
                issue_url: "https://github.com/o/r/issues/1".to_string()
            })
        );
    }

    #[test]
    fn parses_dispatch_dry_run() {
        let cli = Cli::parse_from(["sisyphus", "dispatch", "7", "--dry-run"]);
        assert_eq!(
            cli.command,
            Some(Command::Dispatch {
                queue_item_id: 7,
                dry_run: true,
                repo_path: None
            })
        );
    }

    #[test]
    fn parses_sessions() {
        let cli = Cli::parse_from(["sisyphus", "sessions"]);
        assert_eq!(cli.command, Some(Command::Sessions));
    }

    #[test]
    fn parses_queue_without_subcommand_as_list() {
        let cli = Cli::parse_from(["sisyphus", "queue"]);
        assert_eq!(cli.command, Some(Command::Queue { command: None }));
    }

    #[test]
    fn parses_events() {
        let cli = Cli::parse_from(["sisyphus", "events"]);
        assert_eq!(cli.command, Some(Command::Events));
    }

    #[test]
    fn parses_open() {
        let cli = Cli::parse_from(["sisyphus", "open", "7"]);
        assert_eq!(cli.command, Some(Command::Open { queue_item_id: 7 }));
    }

    #[test]
    fn parses_retry() {
        let cli = Cli::parse_from(["sisyphus", "retry", "7"]);
        assert_eq!(cli.command, Some(Command::Retry { queue_item_id: 7 }));
    }

    #[test]
    fn parses_queue_retry() {
        let cli = Cli::parse_from(["sisyphus", "queue", "retry", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Retry { queue_item_id: 7 })
            })
        );
    }

    #[test]
    fn parses_queue_management_commands() {
        let cli = Cli::parse_from(["sisyphus", "queue", "show", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Show { queue_item_id: 7 })
            })
        );

        let cli = Cli::parse_from(["sisyphus", "queue", "pause", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Pause { queue_item_id: 7 })
            })
        );

        let cli = Cli::parse_from(["sisyphus", "queue", "resume", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Resume { queue_item_id: 7 })
            })
        );

        let cli = Cli::parse_from(["sisyphus", "queue", "cancel", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Cancel { queue_item_id: 7 })
            })
        );

        let cli = Cli::parse_from(["sisyphus", "queue", "remove", "7"]);
        assert_eq!(
            cli.command,
            Some(Command::Queue {
                command: Some(QueueCommand::Remove { queue_item_id: 7 })
            })
        );
    }

    #[test]
    fn parses_repo_add() {
        let cli = Cli::parse_from(["sisyphus", "repo-add", "github", "acme", "widgets", "/repo"]);
        assert_eq!(
            cli.command,
            Some(Command::RepoAdd {
                provider: "github".to_string(),
                owner_or_namespace: "acme".to_string(),
                repo: "widgets".to_string(),
                path: PathBuf::from("/repo"),
                instance_url: None
            })
        );
    }

    #[test]
    fn parses_provider_add() {
        let cli = Cli::parse_from([
            "sisyphus",
            "provider-add",
            "github",
            "acme",
            "widgets",
            "--token-env",
            "GITHUB_TOKEN",
        ]);
        assert_eq!(
            cli.command,
            Some(Command::ProviderAdd {
                provider: "github".to_string(),
                owner_or_namespace: "acme".to_string(),
                repo: "widgets".to_string(),
                token_env: Some("GITHUB_TOKEN".to_string()),
                instance_url: None
            })
        );
    }

    #[test]
    fn parses_provider_add_without_token_env() {
        let cli = Cli::parse_from(["sisyphus", "provider-add", "github", "acme", "widgets"]);
        assert_eq!(
            cli.command,
            Some(Command::ProviderAdd {
                provider: "github".to_string(),
                owner_or_namespace: "acme".to_string(),
                repo: "widgets".to_string(),
                token_env: None,
                instance_url: None
            })
        );
    }

    #[test]
    fn parses_auth_github() {
        let cli = Cli::parse_from(["sisyphus", "auth", "github"]);
        assert_eq!(
            cli.command,
            Some(Command::Auth {
                provider: "github".to_string(),
                client_id: None,
                scopes: Vec::new()
            })
        );
    }

    #[test]
    fn parses_auth_gitlab() {
        let cli = Cli::parse_from(["sisyphus", "auth", "gitlab"]);
        assert_eq!(
            cli.command,
            Some(Command::Auth {
                provider: "gitlab".to_string(),
                client_id: None,
                scopes: Vec::new()
            })
        );
    }

    #[test]
    fn parses_auth_github_oauth_options() {
        let cli = Cli::parse_from([
            "sisyphus",
            "auth",
            "github",
            "--client-id",
            "client-1",
            "--scope",
            "repo",
            "--scope",
            "read:user",
        ]);
        assert_eq!(
            cli.command,
            Some(Command::Auth {
                provider: "github".to_string(),
                client_id: Some("client-1".to_string()),
                scopes: vec!["repo".to_string(), "read:user".to_string()]
            })
        );
    }
}
