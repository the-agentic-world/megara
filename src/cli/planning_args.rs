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
    /// Run the Codex MCP stdio server.
    Mcp(PlanningMcpArgs),
    /// Migrate legacy workflow state and managed projections.
    Migrate(PlanningMigrateArgs),
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
    Spec {
        #[command(subcommand)]
        command: PlanningSpecCommands,
    },
    Plan {
        #[command(subcommand)]
        command: PlanningPlanCommands,
    },
    Export(PlanningExportArgs),
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
pub struct PlanningMcpArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

#[derive(Debug, Args)]
pub struct PlanningMigrateArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub apply: bool,
    #[arg(long)]
    pub resume: Option<String>,
    #[arg(long)]
    pub rollback: Option<String>,
    #[arg(long)]
    pub force: bool,
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
pub enum PlanningSpecCommands {
    Generate(PlanningCandidateGenerateArgs),
    Show(PlanningCandidateShowArgs),
    Approve(PlanningSpecApproveArgs),
    Revise(PlanningCandidateReviseArgs),
}

#[derive(Debug, Subcommand)]
pub enum PlanningPlanCommands {
    Generate(PlanningCandidateGenerateArgs),
    Show(PlanningCandidateShowArgs),
    Approve(PlanningPlanApproveArgs),
    Revise(PlanningCandidateReviseArgs),
}

#[derive(Debug, Args)]
pub struct PlanningCandidateGenerateArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub proposal: String,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningCandidateShowArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long = "candidate")]
    pub candidate_id: Option<String>,
    #[arg(long, value_enum)]
    pub format: Option<PlanningCandidateFormat>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PlanningCandidateFormat {
    Markdown,
    Json,
}

impl PlanningCandidateFormat {
    pub(crate) fn as_wire(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Args)]
pub struct PlanningSpecApproveArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub candidate: String,
    #[arg(long)]
    pub semantic_hash: String,
    #[arg(long)]
    pub base_domain_revision: u64,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningPlanApproveArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub candidate: String,
    #[arg(long)]
    pub semantic_hash: String,
    #[arg(long)]
    pub base_plan_revision: u64,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("revision_input")
        .required(true)
        .args(["text", "read_stdin"])
))]
pub struct PlanningCandidateReviseArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub candidate: String,
    #[arg(long, conflicts_with = "read_stdin")]
    pub text: Option<String>,
    #[arg(long = "stdin")]
    pub read_stdin: bool,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PlanningExportFormatArg {
    Bundle,
    #[value(name = "state-json")]
    StateJson,
    #[value(name = "events-jsonl")]
    EventsJsonl,
}

impl PlanningExportFormatArg {
    pub(crate) fn as_wire(&self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::StateJson => "state-json",
            Self::EventsJsonl => "events-jsonl",
        }
    }
}

#[derive(Debug, Args)]
pub struct PlanningExportArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value = "bundle")]
    pub format: PlanningExportFormatArg,
    #[arg(long)]
    pub include_transcript: bool,
    #[arg(long)]
    pub force: bool,
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
