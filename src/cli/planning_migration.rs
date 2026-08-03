use anyhow::Result;

use super::args::PlanningMigrateArgs;
use crate::planning::migration::{MigrationPhase, MigrationReportAction};

pub(super) fn run(args: PlanningMigrateArgs) -> Result<()> {
    let report = crate::planning::migration::run(crate::planning::migration::MigrationOptions {
        project: args.project,
        dry_run: args.dry_run,
        apply: args.apply,
        resume: args.resume,
        rollback: args.rollback,
        force: args.force,
    })?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "migration {}: phase={}, source_files={}, removed_files={}, session={}",
            report.migration_id,
            phase_name(report.phase),
            report.source_files.len(),
            report.removed_files.len(),
            report.session_id.as_deref().unwrap_or("none")
        );
        for entry in report.entries {
            let action = match entry.action {
                MigrationReportAction::Backup => "backup",
                MigrationReportAction::Remove => "remove",
                MigrationReportAction::Preserve => "preserve",
            };
            if let Some(reason) = entry.reason {
                println!("{action}: {} ({reason})", entry.relative_path.display());
            } else {
                println!("{action}: {}", entry.relative_path.display());
            }
        }
        for warning in report.warnings {
            println!("warning: {warning}");
        }
    }
    Ok(())
}

fn phase_name(phase: MigrationPhase) -> &'static str {
    match phase {
        MigrationPhase::Prepared => "prepared",
        MigrationPhase::PlanningImported => "planning_imported",
        MigrationPhase::ProjectionRemoved => "projection_removed",
        MigrationPhase::Applied => "applied",
        MigrationPhase::RolledBack => "rolled_back",
    }
}
