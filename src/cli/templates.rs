use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum TemplateCommands {
    /// List bundled workflow and agent templates.
    List(JsonArgs),
    /// Print a bundled template by name.
    Show(ShowTemplateArgs),
}

#[derive(Debug, Subcommand)]
pub enum TargetCommands {
    /// List supported target runtimes.
    List(JsonArgs),
}

#[derive(Debug, Args)]
pub struct JsonArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ShowTemplateArgs {
    pub name: String,
}
