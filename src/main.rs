mod agents;
mod cli;
mod docs;
mod doctor;
mod installer;
mod paths;
pub mod planning;
mod targets;
mod templates;
mod tui;
mod ui;
mod uninstall;
mod update;
mod writer;

use anyhow::Result;
use clap::Parser;
use cli::{
    Cli, Commands, DocsCommands, PlanningArgs, PlanningCommands, PlanningSessionArgs,
    PlanningStartArgs, TargetCommands, TemplateCommands,
};
use installer::{InstallAction, InstallOptions, Planner};
use templates::TemplateRegistry;

fn main() -> Result<()> {
    let cli = Cli::parse();
    update::maybe_notify(&cli.command);
    let registry = TemplateRegistry::default();

    match cli.command {
        Commands::Define(args) => cli::run_planning(PlanningArgs {
            command: PlanningCommands::Start(PlanningStartArgs {
                project: args.project,
                request: args.request,
                title: args.title,
                command_id: args.command_id,
                json: args.json,
            }),
        })?,
        Commands::Plan(args) => cli::run_planning(PlanningArgs {
            command: PlanningCommands::Current(PlanningSessionArgs {
                project: args.project,
                session: args.session,
                json: args.json,
            }),
        })?,
        Commands::Install(args) => {
            let Some(args) = tui::prepare_install(args)? else {
                return Ok(());
            };
            let options = InstallOptions::resolve(args, false, InstallAction::Install)?;
            let result = Planner::new(&registry, options).execute()?;
            result.print()?;
        }
        Commands::Sync(args) => {
            for options in InstallOptions::resolve_sync(args)? {
                let result = Planner::new(&registry, options).execute()?;
                result.print()?;
            }
        }
        Commands::Uninstall(args) => uninstall::run(args, &registry)?,
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
        Commands::Planning(args) => cli::run_planning(args)?,
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
