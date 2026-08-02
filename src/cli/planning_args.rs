use std::path::PathBuf;

use clap::{ArgGroup, Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub struct PlanningArgs {
    #[command(subcommand)]
    pub command: PlanningCommands,
}

#[derive(Debug, Subcommand)]
pub enum PlanningCommands {
    /// Read one JSONL request and write one JSONL response.
    Rpc(PlanningRpcArgs),
    Start(PlanningStartArgs),
    Answer(PlanningAnswerArgs),
    Status(PlanningSessionArgs),
    Current(PlanningSessionArgs),
    List(PlanningListArgs),
    Evidence {
        #[command(subcommand)]
        command: PlanningEvidenceCommands,
    },
    Audit {
        #[command(subcommand)]
        command: PlanningAuditCommands,
    },
    Purge(PlanningPurgeArgs),
}

#[derive(Debug, Args)]
pub struct PlanningRpcArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningStartArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub request: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("answer_input")
        .required(true)
        .args(["text", "read_stdin"])
))]
pub struct PlanningAnswerArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub question: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long, conflicts_with = "read_stdin")]
    pub text: Option<String>,
    #[arg(long = "stdin")]
    pub read_stdin: bool,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningSessionArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningListArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub phase: Option<PlanningPhaseArg>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PlanningPhaseArg {
    Interview,
    Specification,
    Planning,
    Complete,
}

impl PlanningPhaseArg {
    pub(crate) fn as_wire(&self) -> &'static str {
        match self {
            Self::Interview => "interview",
            Self::Specification => "specification",
            Self::Planning => "planning",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Args)]
pub struct PlanningPurgeArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub confirm: String,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningEvidenceRefreshArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub citations: String,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PlanningAuditModeArg {
    Delta,
    Full,
}

impl PlanningAuditModeArg {
    pub(crate) fn as_wire(&self) -> &'static str {
        match self {
            Self::Delta => "delta",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Args)]
pub struct PlanningAuditApplyArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub mode: PlanningAuditModeArg,
    #[arg(long)]
    pub proposal: String,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum PlanningEvidenceCommands {
    Refresh(PlanningEvidenceRefreshArgs),
}

#[derive(Debug, Subcommand)]
pub enum PlanningAuditCommands {
    Apply(PlanningAuditApplyArgs),
}
