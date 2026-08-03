use std::{fs, path::Path, process::Command};

use crate::{
    installer::{runtime_support_files, DoctorOptions, MANAGED_MARKER},
    paths::{InstallPaths, TargetRuntime},
    planning::store::PlanningStore,
    targets::{codex, pi},
    templates::TemplateRegistry,
    ui::{self, Section},
};
use anyhow::Result;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub scope: String,
    pub target: String,
    pub ok: bool,
    pub missing: Vec<String>,
    pub unmanaged: Vec<String>,
    pub stale: Vec<String>,
    pub warnings: Vec<String>,
    pub observations: Vec<String>,
    #[serde(skip)]
    pub json: bool,
}

pub fn run(_registry: &TemplateRegistry, options: DoctorOptions) -> Result<DoctorReport> {
    let paths = InstallPaths::resolve(options.scope, options.target)?;
    let mut missing = Vec::new();
    let mut unmanaged = Vec::new();
    let mut stale = Vec::new();
    let mut warnings = runtime_dependency_issues(options.target);
    let mut observations = Vec::new();

    missing.extend(
        TemplateRegistry::missing_paths(&paths.ssot_root)
            .into_iter()
            .map(|path| path.display().to_string()),
    );

    if missing.is_empty() {
        for file in runtime_support_files(paths.ssot_root.clone(), paths.runtime_root.clone())? {
            inspect_managed_file(
                &file.path,
                &file.content,
                &mut missing,
                &mut unmanaged,
                &mut stale,
            )?;
            if file
                .path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                == Some("megara")
            {
                inspect_wrapper_invocation(&file.path, &mut warnings);
            }
        }

        let ssot_registry = TemplateRegistry::from_ssot_root(&paths.ssot_root)?;
        let projection_files = match options.target {
            TargetRuntime::Codex => {
                codex::projection_files(paths.target_root.clone(), options.scope, &ssot_registry)?
            }
            TargetRuntime::Pi => {
                pi::projection_files(paths.target_root.clone(), options.scope, &ssot_registry)?
            }
        };

        for file in projection_files {
            inspect_managed_file(
                &file.path,
                &file.content,
                &mut missing,
                &mut unmanaged,
                &mut stale,
            )?;
        }
        if options.target == TargetRuntime::Pi
            && options.scope == crate::paths::InstallScope::Project
        {
            pi::inspect_trust(
                &paths.runtime_root,
                paths
                    .target_root
                    .parent()
                    .expect("project Pi target root has a parent"),
                &ssot_registry,
                &mut warnings,
            )?;
        }
    }

    inspect_planning_cleanup(
        options.scope,
        &paths.runtime_root,
        options.repair,
        &mut warnings,
        &mut observations,
    )?;

    Ok(DoctorReport {
        scope: options.scope.to_string(),
        target: options.target.to_string(),
        ok: missing.is_empty() && unmanaged.is_empty() && stale.is_empty() && warnings.is_empty(),
        missing,
        unmanaged,
        stale,
        warnings,
        observations,
        json: options.json,
    })
}

fn inspect_planning_cleanup(
    scope: crate::paths::InstallScope,
    runtime_root: &Path,
    repair: bool,
    warnings: &mut Vec<String>,
    observations: &mut Vec<String>,
) -> Result<()> {
    if scope != crate::paths::InstallScope::Project {
        return Ok(());
    }
    let Some(project_root) = runtime_root.parent() else {
        return Ok(());
    };
    let mut store = if repair {
        PlanningStore::open_existing_project_for_repair(project_root)?
    } else {
        PlanningStore::open_existing_project(project_root)?
    };
    let Some(mut store) = store.take() else {
        return Ok(());
    };
    let pending_before = store.pending_cleanup_count()?;
    if pending_before == 0 {
        return Ok(());
    }

    if repair {
        let repaired = store.repair_pending_cleanup()?;
        let pending_after = store.pending_cleanup_count()?;
        observations.push(format!(
            "Planning purge cleanup repair: repaired={repaired}, pending={pending_after}"
        ));
        if pending_after > 0 {
            warnings.push(format!(
                "pending Planning purge cleanup remains: {pending_after}; retry `megara doctor --repair`"
            ));
        }
    } else {
        warnings.push(format!(
            "pending Planning purge cleanup: {pending_before}; run `megara doctor --repair`"
        ));
    }
    Ok(())
}

impl DoctorReport {
    pub fn print(&self) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }

        let rows = [
            ("scope", self.scope.clone()),
            ("target", self.target.clone()),
            ("ok", self.ok.to_string()),
        ];
        let mut sections = vec![Section::new(
            "Run",
            vec![format!(
                "megara doctor: scope={}, target={}, ok={}",
                self.scope, self.target, self.ok
            )],
        )];
        push_group(&mut sections, "Missing", &self.missing);
        push_group(&mut sections, "Unmanaged", &self.unmanaged);
        push_group(&mut sections, "Stale", &self.stale);
        push_group(&mut sections, "Warnings", &self.warnings);
        push_group(&mut sections, "Observations", &self.observations);

        let status = if self.ok { "OK" } else { "issues found" };
        ui::print_dashboard("Doctor", status, &rows, &sections)?;
        Ok(())
    }
}

fn runtime_dependency_issues(target: TargetRuntime) -> Vec<String> {
    match target {
        TargetRuntime::Codex => codex::runtime_dependency_issues(),
        TargetRuntime::Pi => pi::runtime_dependency_issues(),
    }
}

fn push_group(sections: &mut Vec<Section>, label: &str, paths: &[String]) {
    if !paths.is_empty() {
        sections.push(Section::new(label, paths.to_vec()));
    }
}

fn inspect_managed_file(
    path: &Path,
    desired: &str,
    missing: &mut Vec<String>,
    unmanaged: &mut Vec<String>,
    stale: &mut Vec<String>,
) -> Result<()> {
    if !path.exists() {
        missing.push(path.display().to_string());
        return Ok(());
    }

    let current = fs::read_to_string(path)?;
    if !current.contains(MANAGED_MARKER) {
        unmanaged.push(path.display().to_string());
    } else if current != desired {
        stale.push(path.display().to_string());
    }
    Ok(())
}

fn inspect_wrapper_invocation(path: &Path, warnings: &mut Vec<String>) {
    if !path.exists() {
        return;
    }
    match Command::new(path).arg("--version").output() {
        Ok(output) if output.status.success() => {}
        Ok(output) => warnings.push(format!(
            "Megara wrapper is not invocable: {} exited with {}",
            path.display(),
            output.status
        )),
        Err(error) => warnings.push(format!(
            "Megara wrapper is not invocable: {} ({error})",
            path.display()
        )),
    }
}
