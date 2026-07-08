use clap::{Parser, Subcommand};

#[path = "cli/common.rs"]
mod common;
#[path = "cli/docs.rs"]
mod docs;
#[path = "cli/install.rs"]
mod install;
#[path = "cli/resolve.rs"]
mod resolve;
#[path = "cli/templates.rs"]
mod templates;
#[path = "cli/ultragoal.rs"]
mod ultragoal;
#[path = "cli/update.rs"]
mod update;

pub use common::{ScopeArg, TargetArg};
#[allow(unused_imports)]
pub use docs::{DocsArgs, DocsCheckArgs, DocsCommands, DocsInitArgs};
pub use install::{DoctorArgs, HookArgs, InstallArgs, SyncArgs};
pub use resolve::{resolve_scope, resolve_target};
pub use templates::{TargetCommands, TemplateCommands};
pub use ultragoal::{
    UltragoalArgs, UltragoalCheckpointArgs, UltragoalCommands, UltragoalCreateGoalsArgs,
    UltragoalGoalStatusArg, UltragoalStartGoalArgs, UltragoalStatusArgs, UltragoalSteerArgs,
    UltragoalSteerKindArg,
};
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
    /// Run the harness installer wizard.
    Install(InstallArgs),
    /// Reproject managed runtime files from the Megara SSOT.
    Sync(SyncArgs),
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
    /// Manage durable goal execution state.
    Ultragoal(UltragoalArgs),
    /// Manage OKF knowledge bundles.
    Docs(DocsArgs),
    /// Update the Megara binary and installed harness files.
    Update(UpdateArgs),
    /// Internal runtime hook entrypoint.
    #[command(hide = true)]
    Hook(HookArgs),
}
