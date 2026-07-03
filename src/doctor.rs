use std::{collections::BTreeMap, fs, path::Path, process::Command};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::{
    installer::{runtime_support_files, DoctorOptions, MANAGED_MARKER},
    paths::{InstallPaths, TargetRuntime},
    targets::codex,
    templates::TemplateRegistry,
    ui::{self, Section},
};

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
        for file in runtime_support_files(paths.ssot_root.clone())? {
            inspect_managed_file(
                &file.path,
                &file.content,
                &mut missing,
                &mut unmanaged,
                &mut stale,
            )?;
            inspect_wrapper_invocation(&file.path, &mut warnings);
        }

        let ssot_registry = TemplateRegistry::from_ssot_root(&paths.ssot_root)?;
        let projection_files = match options.target {
            TargetRuntime::Codex => {
                codex::projection_files(paths.target_root, options.scope, &ssot_registry)?
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
        inspect_hook_events(&paths.ssot_root, &mut observations);
        inspect_stale_workflows(&paths.ssot_root, &mut warnings)?;
    }

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

fn inspect_hook_events(ssot_root: &Path, observations: &mut Vec<String>) {
    let events = ssot_root.join("state/hooks/events.jsonl");
    if events.exists() {
        observations.push(format!(
            "Codex hook events observed at {}",
            events.display()
        ));
    } else {
        observations.push(format!(
            "Codex hook events have not been observed yet at {}",
            events.display()
        ));
    }
}

fn inspect_stale_workflows(ssot_root: &Path, warnings: &mut Vec<String>) -> Result<()> {
    let workflow_dir = ssot_root.join("state/workflows/deep-interview");
    if !workflow_dir.exists() {
        return Ok(());
    }

    let mut states = Vec::new();
    for entry in fs::read_dir(workflow_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_json_file(&path) {
            continue;
        }
        let state: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
        states.push((path, state));
    }

    for (path, state) in &states {
        let status = state.get("status").and_then(serde_json::Value::as_str);
        let phase = state.get("phase").and_then(serde_json::Value::as_str);
        if matches!(status.or(phase), Some("stale")) {
            if let Some(superseded_by) = state.get("stale_superseded_by").and_then(Value::as_str) {
                warnings.push(format!(
                    "stale deep-interview alias state: {} -> {}",
                    path.display(),
                    superseded_by
                ));
            } else {
                warnings.push(format!("stale deep-interview state: {}", path.display()));
            }
            continue;
        }
        if active_pending(state) && terminal_peer_exists(path)? {
            warnings.push(format!("stale deep-interview state: {}", path.display()));
        }
    }
    inspect_duplicate_active_deep_interviews(&states, warnings);
    Ok(())
}

fn terminal_peer_exists(path: &Path) -> Result<bool> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let current = fs::read_to_string(path)?;
    let current: Value = serde_json::from_str(&current)?;
    let cwd = current.get("cwd").cloned().unwrap_or(Value::Null);
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let peer = entry.path();
        if peer == path || !is_json_file(&peer) {
            continue;
        }
        let state: Value = serde_json::from_str(&fs::read_to_string(peer)?)?;
        let same_cwd = state.get("cwd").cloned().unwrap_or(Value::Null) == cwd;
        let active = state.get("active").and_then(Value::as_bool).unwrap_or(true);
        if same_cwd && !active {
            return Ok(true);
        }
    }
    Ok(false)
}

fn active_pending(state: &Value) -> bool {
    state
        .get("active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && state
            .get("pending_question")
            .and_then(|pending| pending.get("status"))
            .and_then(Value::as_str)
            == Some("pending")
}

fn is_json_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|extension| extension == "json")
}

fn inspect_duplicate_active_deep_interviews(
    states: &[(std::path::PathBuf, Value)],
    warnings: &mut Vec<String>,
) {
    let mut active_by_cwd = BTreeMap::<String, Vec<String>>::new();
    for (path, state) in states {
        if !state
            .get("active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let cwd = state
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or("<unknown cwd>")
            .to_string();
        active_by_cwd
            .entry(cwd)
            .or_default()
            .push(path.display().to_string());
    }

    for (cwd, paths) in active_by_cwd {
        if paths.len() < 2 {
            continue;
        }
        warnings.push(format!(
            "duplicate active deep-interview states for cwd {cwd}: {}",
            paths.join(", ")
        ));
    }
}
