use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[path = "cli/agents.rs"]
mod agents;
#[path = "cli/common.rs"]
mod common;
#[path = "cli/docs.rs"]
mod docs;
#[path = "cli/install.rs"]
mod install;
#[path = "cli/pi.rs"]
mod pi;
#[path = "cli/planning.rs"]
mod planning;
#[path = "cli/resolve.rs"]
mod resolve;
#[path = "cli/team.rs"]
mod team;
#[path = "cli/templates.rs"]
mod templates;
#[path = "cli/ultragoal.rs"]
mod ultragoal;
#[path = "cli/uninstall.rs"]
mod uninstall;
#[path = "cli/update.rs"]
mod update;

pub use agents::{
    AgentsArgs, AgentsCommands, ConfigureAgentsArgs, ResetAgentsArgs, ShowAgentsArgs,
};
pub use common::{ScopeArg, TargetArg};
#[allow(unused_imports)]
pub use docs::{DocsArgs, DocsCheckArgs, DocsCommands, DocsInitArgs};
pub use install::{DoctorArgs, HookArgs, InstallArgs, SyncArgs};
#[allow(unused_imports)]
pub use pi::{PiArgs, PiCommands, PiEventArgs};
#[allow(unused_imports)]
pub(crate) use planning::run as run_planning;
#[allow(unused_imports)]
pub use planning::{PlanningArgs, PlanningCommands, PlanningSessionArgs, PlanningStartArgs};

#[derive(Debug, Args)]
pub struct DefineArgs {
    pub request: String,
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanAliasArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub json: bool,
}
pub use resolve::{resolve_scope, resolve_target};
#[allow(unused_imports)]
pub use team::{TeamArgs, TeamCommands, TeamSplitArgs, TeamTeammateArgs};
pub use templates::{TargetCommands, TemplateCommands};
pub use ultragoal::{
    UltragoalArgs, UltragoalCheckpointArgs, UltragoalCommands, UltragoalCreateGoalsArgs,
    UltragoalGoalStatusArg, UltragoalStartGoalArgs, UltragoalStatusArgs, UltragoalSteerArgs,
    UltragoalSteerKindArg,
};
pub use uninstall::UninstallArgs;
pub use update::UpdateArgs;
#[allow(unused_imports)]
pub(crate) use update::UpdateScopeArg;

#[derive(Debug, Parser)]
#[command(name = "megara", version, about = "Install portable agent harnesses")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start a deterministic planning session for a request.
    Define(DefineArgs),
    /// Show current planning state and next action.
    Plan(PlanAliasArgs),
    /// Run the harness installer wizard.
    Install(InstallArgs),
    /// Reproject managed runtime files from the Megara SSOT.
    Sync(SyncArgs),
    /// Remove managed harness files while preserving runtime data.
    Uninstall(UninstallArgs),
    /// Inspect installation health and drift.
    Doctor(DoctorArgs),
    /// Inspect bundled harness templates.
    Templates {
        #[command(subcommand)]
        command: TemplateCommands,
    },
    /// Inspect supported agent runtimes.
    Targets {
        #[command(subcommand)]
        command: TargetCommands,
    },
    /// Configure role model and reasoning policies.
    Agents(AgentsArgs),
    /// Manage durable goal execution state.
    Ultragoal(UltragoalArgs),
    /// Prepare and run team workflow helpers.
    Team(TeamArgs),
    /// Manage OKF knowledge bundles.
    Docs(DocsArgs),
    /// Update the Megara binary and installed harness files.
    Update(UpdateArgs),
    /// Run the Pi Coding Agent extension bridge.
    Pi(PiArgs),
    /// Read and mutate deterministic Planning Core state.
    Planning(PlanningArgs),
    /// Internal runtime hook entrypoint.
    #[command(hide = true)]
    Hook(HookArgs),
}
