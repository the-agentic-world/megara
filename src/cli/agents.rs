use clap::{Args, Subcommand};

use super::{ScopeArg, TargetArg};

#[derive(Debug, Args)]
pub struct AgentsArgs {
    #[command(subcommand)]
    pub command: AgentsCommands,
}

#[derive(Debug, Subcommand)]
pub enum AgentsCommands {
    /// Configure runtime-specific role models and reasoning levels.
    Configure(ConfigureAgentsArgs),
    /// Show effective role policy for a target runtime.
    Show(ShowAgentsArgs),
    /// Remove role overrides from one installation scope.
    Reset(ResetAgentsArgs),
}

#[derive(Debug, Args)]
pub struct ConfigureAgentsArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long, value_delimiter = ',')]
    pub role: Vec<String>,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub reasoning_effort: Option<String>,
    #[arg(long)]
    pub thinking_level: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long, help = "Allow replacing an unmanaged Megara configuration")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ShowAgentsArgs {
    #[arg(long, value_enum)]
    pub scope: ScopeArg,
    #[arg(long, value_enum)]
    pub target: TargetArg,
    #[arg(long, value_delimiter = ',')]
    pub role: Vec<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ResetAgentsArgs {
    #[arg(long, value_enum)]
    pub scope: ScopeArg,
    #[arg(long, value_enum)]
    pub target: TargetArg,
    #[arg(long, value_delimiter = ',')]
    pub role: Vec<String>,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long, help = "Allow replacing an unmanaged Megara configuration")]
    pub force: bool,
}
