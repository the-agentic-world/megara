mod agents;
mod cli;
mod docs;
mod doctor;
mod hook;
mod installer;
mod paths;
mod pi;
mod targets;
mod team;
mod templates;
mod tui;
mod ui;
mod ultragoal;
mod update;
mod writer;

use anyhow::Result;
use clap::Parser;
use cli::{
    Cli, Commands, DocsCommands, PiCommands, TargetCommands, TeamCommands, TemplateCommands,
};
use installer::{InstallAction, InstallOptions, Planner};
use templates::TemplateRegistry;

fn main() -> Result<()> {
    let cli = Cli::parse();
    update::maybe_notify(&cli.command);
    let registry = TemplateRegistry::default();

    match cli.command {
        Commands::Install(args) => {
            let Some(args) = tui::prepare_install(args)? else {
                return Ok(());
            };
            let options = InstallOptions::resolve(args, false, InstallAction::Install)?;
            let result = Planner::new(&registry, options).execute()?;
            result.print()?;
        }
        Commands::Sync(args) => {
            let options = InstallOptions::resolve(args.into(), false, InstallAction::Sync)?;
            let result = Planner::new(&registry, options).execute()?;
            result.print()?;
        }
        Commands::Doctor(args) => {
            let use_tui = tui::use_doctor_tui(&args);
            let options = if use_tui {
                tui::doctor_tui_options(args)?
            } else {
                args.resolve()?
            };
            let report = doctor::run(&registry, options)?;
            if use_tui {
                tui::show_doctor_report(&report)?;
            } else {
                report.print()?;
            }
        }
        Commands::Agents(args) => agents::run(args, &registry)?,
        Commands::Hook(args) => {
            let exit_code = hook::run(args)?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
        Commands::Ultragoal(args) => ultragoal::run(args)?,
        Commands::Team(args) => match args.command {
            TeamCommands::Split(args) => {
                let report = team::split::prepare_from_cli(args)?;
                if report.json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    report.print()?;
                }
            }
            TeamCommands::Teammate(args) => {
                team::split::run_teammate_from_cli(args)?;
            }
        },
        Commands::Docs(args) => match args.command {
            DocsCommands::Init(args) => docs::init(args)?,
            DocsCommands::Check(args) => docs::check(args)?,
        },
        Commands::Update(args) => {
            if tui::use_update_tui(&args) && !tui::confirm_update(&args)? {
                return Ok(());
            }
            update::run(args)?
        }
        Commands::Pi(args) => match args.command {
            PiCommands::Event(args) => pi::run_event(args, &registry)?,
        },
        Commands::Templates { command } => match command {
            TemplateCommands::List(args) => {
                let list = registry.template_names();
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&list)?);
                } else {
                    ui::print_list("Templates", "Bundled harness templates", &list)?;
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
                    let rows = targets
                        .into_iter()
                        .map(|target| format!("{} · {}", target.name, target.status))
                        .collect::<Vec<_>>();
                    ui::print_list("Targets", "Supported agent runtimes", &rows)?;
                }
            }
        },
    }

    Ok(())
}
