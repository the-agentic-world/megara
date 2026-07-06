use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct DocsArgs {
    #[command(subcommand)]
    pub command: DocsCommands,
}

#[derive(Debug, Subcommand)]
pub enum DocsCommands {
    /// Create an OKF knowledge bundle scaffold.
    Init(DocsInitArgs),
    /// Check an OKF knowledge bundle.
    Check(DocsCheckArgs),
}

#[derive(Debug, Args)]
pub struct DocsInitArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DocsCheckArgs {
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}
