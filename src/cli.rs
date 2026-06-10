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
    #[command(about = "List local queued work items")]
    Queue,
    #[command(about = "List agent session references known to Sisyphus")]
    Sessions,
    #[command(about = "List local lifecycle events")]
    Events,
    #[command(about = "Open or print a Codex-native session reference")]
    Open { queue_item_id: i64 },
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
    #[command(
        alias = "resgister",
        about = "Register reboot-persistent autostart for `sisyphus serve --daemon`"
    )]
    Register,
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
}
