use anyhow::Result;

use super::model::{InstallAction, InstallResult};

impl InstallResult {
    pub fn print(&self) -> Result<()> {
        if self.options.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }

        let verb = match (self.options.action, self.options.dry_run) {
            (InstallAction::Install, true) => "install planned",
            (InstallAction::Install, false) => "installed",
            (InstallAction::Sync, true) => "sync planned",
            (InstallAction::Sync, false) => "synced",
        };
        println!(
            "megara {verb}: scope={}, target={}, ssot={}, projection={}",
            self.plan.scope,
            self.plan.target,
            self.plan.ssot_root.display(),
            self.plan.target_root.display()
        );
        println!(
            "created={}, updated={}, unchanged={}, conflicts={}",
            self.summary.created.len(),
            self.summary.updated.len(),
            self.summary.unchanged.len(),
            self.summary.conflicts.len()
        );

        print_conflicts(self);
        print_hook_trust(self);
        print_warnings(self);
        Ok(())
    }
}

fn print_conflicts(result: &InstallResult) {
    if result.summary.conflicts.is_empty() {
        return;
    }
    println!("conflicts:");
    for path in &result.summary.conflicts {
        println!("- {}", path.display());
    }
}

fn print_hook_trust(result: &InstallResult) {
    let Some(hook_trust) = &result.hook_trust else {
        return;
    };
    println!(
        "hook trust: registered={}, unchanged={}, skipped={}, config={}",
        hook_trust.registered,
        hook_trust.unchanged,
        hook_trust.skipped,
        hook_trust.config_path.display()
    );
}

fn print_warnings(result: &InstallResult) {
    if result.warnings.is_empty() {
        return;
    }
    println!("warnings:");
    for warning in &result.warnings {
        println!("- {warning}");
    }
}
