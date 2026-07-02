use clap::{Parser, Subcommand};

#[path = "cli/common.rs"]
mod common;
#[path = "cli/install.rs"]
mod install;
#[path = "cli/resolve.rs"]
mod resolve;
#[path = "cli/templates.rs"]
mod templates;
#[path = "cli/ultragoal.rs"]
mod ultragoal;

pub use common::{ScopeArg, TargetArg};
pub use install::{DoctorArgs, HookArgs, InstallArgs};
pub use resolve::{resolve_scope, resolve_target};
pub use templates::{TargetCommands, TemplateCommands};
pub use ultragoal::{
    UltragoalArgs, UltragoalCheckpointArgs, UltragoalCommands, UltragoalCompleteGoalsArgs,
    UltragoalCreateGoalsArgs, UltragoalGoalStatusArg, UltragoalStatusArgs, UltragoalSteerArgs,
    UltragoalSteerKindArg,
};

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
    Sync(InstallArgs),
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
    /// Internal runtime hook entrypoint.
    #[command(hide = true)]
    Hook(HookArgs),
}
