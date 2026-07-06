use anyhow::Result;

use crate::ui::{self, Section};

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
        let rows = [
            ("scope", self.plan.scope.to_string()),
            ("target", self.plan.target.to_string()),
            ("ssot", self.plan.ssot_root.display().to_string()),
            ("runtime", self.plan.runtime_root.display().to_string()),
            ("projection", self.plan.target_root.display().to_string()),
        ];
        let mut sections = vec![Section::new(
            "Run",
            vec![
                format!(
                    "megara {verb}: scope={}, target={}, ssot={}, runtime={}, projection={}",
                    self.plan.scope,
                    self.plan.target,
                    self.plan.ssot_root.display(),
                    self.plan.runtime_root.display(),
                    self.plan.target_root.display()
                ),
                format!(
                    "created={}, updated={}, unchanged={}, conflicts={}, removed={}",
                    self.summary.created.len(),
                    self.summary.updated.len(),
                    self.summary.unchanged.len(),
                    self.summary.conflicts.len(),
                    self.summary.removed.len()
                ),
            ],
        )];

        if !self.summary.conflicts.is_empty() {
            sections.push(Section::new(
                "Conflicts",
                self.summary
                    .conflicts
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            ));
        }

        if !self.summary.removed.is_empty() {
            sections.push(Section::new(
                "Removed",
                self.summary
                    .removed
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            ));
        }

        if let Some(hook_trust) = &self.hook_trust {
            sections.push(Section::new(
                "Hook Trust",
                vec![format!(
                    "hook trust: registered={}, unchanged={}, skipped={}, config={}",
                    hook_trust.registered,
                    hook_trust.unchanged,
                    hook_trust.skipped,
                    hook_trust.config_path.display()
                )],
            ));
        }

        if !self.warnings.is_empty() {
            sections.push(Section::new("Warnings", self.warnings.clone()));
        }

        ui::print_dashboard("Install", verb, &rows, &sections)?;
        Ok(())
    }
}
