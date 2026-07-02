mod cli;
mod doctor;
mod hook;
mod installer;
mod paths;
mod targets;
mod templates;
mod ultragoal;
mod update;
mod writer;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, TargetCommands, TemplateCommands};
use installer::{InstallAction, InstallOptions, Planner};
use templates::TemplateRegistry;

fn main() -> Result<()> {
    let cli = Cli::parse();
    update::maybe_notify(&cli.command);
    let registry = TemplateRegistry::default();

    match cli.command {
        Commands::Install(args) => {
            let options = InstallOptions::resolve(args, true, InstallAction::Install)?;
            let result = Planner::new(&registry, options).execute()?;
            result.print()?;
        }
        Commands::Sync(args) => {
            let options = InstallOptions::resolve(args, false, InstallAction::Sync)?;
            let result = Planner::new(&registry, options).execute()?;
            result.print()?;
        }
        Commands::Doctor(args) => {
            let options = args.resolve()?;
            let report = doctor::run(&registry, options)?;
            report.print()?;
        }
        Commands::Hook(args) => {
            let exit_code = hook::run(args)?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Commands::Ultragoal(args) => ultragoal::run(args)?,
        Commands::Update(args) => update::run(args)?,
        Commands::Templates { command } => match command {
            TemplateCommands::List(args) => {
                let list = registry.template_names();
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&list)?);
                } else {
                    for name in list {
                        println!("{name}");
                    }
                }
            }
            TemplateCommands::Show(args) => {
                let template = registry
                    .find(&args.name)
                    .ok_or_else(|| anyhow::anyhow!("unknown template: {}", args.name))?;
                println!("{}", template.content);
            }
        },
        Commands::Targets { command } => match command {
            TargetCommands::List(args) => {
                let targets = targets::supported_targets();
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&targets)?);
                } else {
                    for target in targets {
                        println!("{}", target.name);
                    }
                }
            }
        },
    }

    Ok(())
}
