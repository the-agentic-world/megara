use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct TeamArgs {
    #[command(subcommand)]
    pub command: TeamCommands,
}

#[derive(Debug, Subcommand)]
pub enum TeamCommands {
    /// Prepare CLI split panes for teammate agents.
    Split(TeamSplitArgs),
    /// Internal split-pane teammate runner.
    #[command(hide = true)]
    Teammate(TeamTeammateArgs),
}

#[derive(Debug, Args)]
pub struct TeamSplitArgs {
    #[arg(long, value_delimiter = ',', required = true)]
    pub roles: Vec<String>,
    #[arg(long, default_value = "auto", help = "auto, cmux, tmux, or orca")]
    pub transport: String,
    #[arg(long)]
    pub correlation_id: String,
    #[arg(long)]
    pub task: Option<String>,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    #[arg(long)]
    pub runtime_root: Option<PathBuf>,
    #[arg(long, default_value = "codex")]
    pub codex_bin: String,
    #[arg(long)]
    pub megara_bin: Option<String>,
    #[arg(long)]
    pub open: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TeamTeammateArgs {
    #[arg(long)]
    pub transport: Option<String>,
    #[arg(long)]
    pub role: String,
    #[arg(long)]
    pub teammate_id: String,
    #[arg(long)]
    pub correlation_id: String,
    #[arg(long)]
    pub assignment_file: PathBuf,
    #[arg(long)]
    pub receipt_dir: PathBuf,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    #[arg(long, default_value = "codex")]
    pub codex_bin: String,
}
