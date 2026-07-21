use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::ScopeArg;

#[derive(Debug, Args)]
pub struct PiArgs {
    #[command(subcommand)]
    pub command: PiCommands,
}

#[derive(Debug, Subcommand)]
pub enum PiCommands {
    /// Exchange one Pi extension runtime event over standard input/output.
    Event(PiEventArgs),
}

#[derive(Debug, Args)]
pub struct PiEventArgs {
    #[arg(long, value_enum, default_value = "project")]
    pub scope: ScopeArg,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
}
