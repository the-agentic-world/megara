use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use super::ScopeArg;

#[derive(Debug, Args)]
pub struct UltragoalArgs {
    #[arg(long, value_enum, default_value = "project")]
    pub scope: ScopeArg,
    #[arg(long, default_value = "default")]
    pub session_id: String,
    #[command(subcommand)]
    pub command: UltragoalCommands,
}

#[derive(Debug, Subcommand)]
pub enum UltragoalCommands {
    /// Show durable goal execution status.
    Status(UltragoalStatusArgs),
    /// Create durable goals from an approved ralplan handoff.
    CreateGoals(UltragoalCreateGoalsArgs),
    /// Select or resume the next goal.
    CompleteGoals(UltragoalCompleteGoalsArgs),
    /// Record a goal checkpoint and evidence.
    Checkpoint(UltragoalCheckpointArgs),
    /// Add controlled steering information.
    Steer(UltragoalSteerArgs),
}

#[derive(Debug, Args)]
pub struct UltragoalStatusArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct UltragoalCreateGoalsArgs {
    #[arg(long, conflicts_with_all = ["brief_file", "from_stdin"])]
    pub brief: Option<String>,
    #[arg(long, conflicts_with_all = ["brief", "from_stdin"])]
    pub brief_file: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["brief", "brief_file"])]
    pub from_stdin: bool,
    #[arg(long)]
    pub allow_direct: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct UltragoalCompleteGoalsArgs {
    #[arg(long)]
    pub retry_failed: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct UltragoalCheckpointArgs {
    #[arg(long)]
    pub goal_id: String,
    #[arg(long, value_enum)]
    pub status: UltragoalGoalStatusArg,
    #[arg(long)]
    pub evidence: String,
    #[arg(long)]
    pub quality_gate_json: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum UltragoalGoalStatusArg {
    Pending,
    Active,
    Complete,
    Failed,
    Blocked,
    ReviewBlocked,
    Superseded,
}

#[derive(Debug, Args)]
pub struct UltragoalSteerArgs {
    #[arg(long, value_enum)]
    pub kind: UltragoalSteerKindArg,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub objective: Option<String>,
    #[arg(long)]
    pub evidence: Option<String>,
    #[arg(long)]
    pub rationale: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum UltragoalSteerKindArg {
    AddSubgoal,
    AnnotateLedger,
}
