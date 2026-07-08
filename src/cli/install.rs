use std::path::PathBuf;

use clap::Args;

use super::{ScopeArg, TargetArg};

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long, value_name = "LOCALE", help = "Set user-facing response locale")]
    pub locale: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub no_interactive: bool,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub no_interactive: bool,
}

impl From<SyncArgs> for InstallArgs {
    fn from(args: SyncArgs) -> Self {
        Self {
            scope: args.scope,
            target: args.target,
            locale: None,
            dry_run: args.dry_run,
            force: args.force,
            json: args.json,
            no_interactive: args.no_interactive,
        }
    }
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub no_interactive: bool,
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[arg(long, hide = true)]
    pub managed_marker: Option<String>,
    #[arg(long, value_enum, default_value = "project")]
    pub scope: ScopeArg,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
    #[arg(long, default_value = "codex")]
    pub runtime: String,
    #[arg(long)]
    pub event: String,
    #[arg(long)]
    pub matcher: Option<String>,
}
