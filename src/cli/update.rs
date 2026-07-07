use clap::{Args, ValueEnum};
use serde::Serialize;

use super::TargetArg;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateScopeArg {
    All,
    Global,
    Project,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(long, value_enum, default_value = "all")]
    pub scope: UpdateScopeArg,
    #[arg(long, value_enum, default_value = "codex")]
    pub target: TargetArg,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub no_interactive: bool,
}
