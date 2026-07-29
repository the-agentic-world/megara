use clap::Args;

use super::{ScopeArg, TargetArg};

#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[arg(long, value_enum)]
    pub scope: ScopeArg,
    #[arg(long, value_enum, default_value = "codex")]
    pub target: TargetArg,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub json: bool,
}
