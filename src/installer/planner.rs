use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use toml::Value;

use crate::{
    paths::{InstallPaths, TargetRuntime},
    targets::codex,
    templates::TemplateRegistry,
    writer::{remove_managed_files, write_files},
};

use super::migration;
use super::model::*;

pub struct Planner<'a> {
    registry: &'a TemplateRegistry,
    options: InstallOptions,
}

impl<'a> Planner<'a> {
    pub fn new(registry: &'a TemplateRegistry, options: InstallOptions) -> Self {
        Self { registry, options }
    }

    pub fn plan(&self) -> Result<InstallPlan> {
        let paths = InstallPaths::resolve(self.options.scope, self.options.target)?;
        let mut files = Vec::new();
        files.extend(runtime_support_files(
            paths.ssot_root.clone(),
            paths.runtime_root.clone(),
        )?);
        let projection_registry = match self.options.action {
            InstallAction::Install => {
                let registry = registry_with_locale(self.registry, self.options.locale.as_deref())?;
                files.extend(ssot_files(paths.ssot_root.clone(), &registry));
                registry
            }
            InstallAction::Sync => TemplateRegistry::from_ssot_root(&paths.ssot_root)?,
        };

        match self.options.target {
            TargetRuntime::Codex => files.extend(codex::projection_files(
                paths.target_root.clone(),
                self.options.scope,
                &projection_registry,
            )?),
        };
        let obsolete_files = match self.options.target {
            TargetRuntime::Codex => codex::obsolete_projection_files(
                paths.target_root.clone(),
                self.options.scope,
                &projection_registry,
            ),
        };

        Ok(InstallPlan {
            scope: self.options.scope,
            target: self.options.target,
            ssot_root: paths.ssot_root,
            runtime_root: paths.runtime_root,
            target_root: paths.target_root,
            files,
            obsolete_files,
        })
    }

    pub fn execute(&self) -> Result<InstallResult> {
        let plan = self.plan()?;
        let mut summary = write_files(&plan.files, self.options.dry_run, self.options.force)?;
        summary.removed.extend(remove_managed_files(
            &plan.obsolete_files,
            self.options.dry_run,
        )?);
        let migrations = migration::migrate_legacy_project_state(
            &plan.ssot_root,
            &plan.runtime_root,
            self.options.dry_run,
        )?
        .into_iter()
        .collect::<Vec<_>>();
        let hook_trust = match self.options.target {
            TargetRuntime::Codex => Some(codex::ensure_hook_trust(
                &plan.target_root.join("hooks.json"),
                self.options.dry_run,
            )?),
        };
        let mut warnings = runtime_dependency_issues(self.options.target);
        for migration in &migrations {
            if !migration.conflicts.is_empty() {
                warnings.push(format!(
                    "legacy runtime state migration left {} conflicting file(s) under {}; review them before removing the legacy state directory",
                    migration.conflicts.len(),
                    migration.source.display()
                ));
            }
        }
        if matches!(self.options.action, InstallAction::Install)
            && self.options.target == TargetRuntime::Codex
        {
            warnings.push(
                "Codex App loads hooks when a session starts; open a new session after install for hooks to take effect."
                    .to_string(),
            );
        }
        Ok(InstallResult {
            options: self.options.clone(),
            plan,
            summary,
            migrations,
            hook_trust,
            warnings,
        })
    }
}

fn runtime_dependency_issues(target: TargetRuntime) -> Vec<String> {
    match target {
        TargetRuntime::Codex => codex::runtime_dependency_issues(),
    }
}

fn ssot_files(root: PathBuf, registry: &TemplateRegistry) -> Vec<PlannedFile> {
    registry
        .ssot_files()
        .iter()
        .map(|template| {
            PlannedFile::new(root.join(&template.relative_path), template.content.clone())
        })
        .collect()
}

fn registry_with_locale(
    registry: &TemplateRegistry,
    locale: Option<&str>,
) -> Result<TemplateRegistry> {
    let Some(locale) = locale else {
        return Ok(registry.clone());
    };
    let Some(config) = registry.config() else {
        return Ok(registry.clone());
    };
    let content = render_config_template(&config.content, Some(locale))?;
    Ok(registry.with_config_content(content))
}

fn render_config_template(content: &str, locale: Option<&str>) -> Result<String> {
    let Some(locale) = locale else {
        return Ok(content.to_string());
    };
    let mut value: Value = content
        .parse()
        .context("failed to parse bundled Megara config template")?;
    if let Some(table) = value.as_table_mut() {
        table.insert("locale".to_string(), Value::String(locale.to_string()));
    }
    toml::to_string_pretty(&value).context("failed to render Megara config template")
}

pub(crate) fn runtime_support_files(
    root: PathBuf,
    runtime_root: PathBuf,
) -> Result<Vec<PlannedFile>> {
    let megara_bin = env::current_exe().context("failed to resolve current megara executable")?;
    let mut files = vec![
        PlannedFile::new_executable_shell(
            root.join("bin").join("megara"),
            format!(
                "#!/bin/sh\nexec {} \"$@\"\n",
                shell_quote(&megara_bin.display().to_string())
            ),
        ),
        PlannedFile::new_executable_shell(
            root.join("bin").join("insane-search"),
            r#"#!/bin/sh
set -eu
bin_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
root_dir="$bin_dir/.."
tool_dir="$bin_dir/../tools/insane-search"
if [ "$(basename "$root_dir")" = ".agents" ]; then
  runtime_root="$root_dir/../.megara"
else
  runtime_root="$root_dir"
fi
state_dir="$runtime_root/state/tools/insane-search"
venv_dir="$state_dir/venv"
python_bin="$venv_dir/bin/python"
requirements="$tool_dir/requirements.txt"
requirements_stamp="$state_dir/requirements.stamp"
if [ ! -d "$tool_dir" ]; then
  echo "insane-search tool directory not found: $tool_dir" >&2
  exit 2
fi
if [ ! -x "$python_bin" ]; then
  mkdir -p "$state_dir"
  echo "insane-search: bootstrapping Python dependencies into $venv_dir" >&2
  python3 -m venv "$venv_dir"
fi
needs_install=0
if [ ! -f "$requirements_stamp" ] || [ "$requirements" -nt "$requirements_stamp" ]; then
  needs_install=1
fi
if ! "$python_bin" - <<'PY' >/dev/null 2>&1
import importlib.util
missing = [
    package
    for package in ("curl_cffi", "bs4", "yaml", "yt_dlp")
    if importlib.util.find_spec(package) is None
]
raise SystemExit(1 if missing else 0)
PY
then
  needs_install=1
fi
if [ "$needs_install" = "1" ]; then
  "$python_bin" -m ensurepip --upgrade >/dev/null 2>&1 || true
  PIP_DISABLE_PIP_VERSION_CHECK=1 "$python_bin" -m pip install -r "$requirements" >&2
  touch "$requirements_stamp"
fi
cd "$tool_dir"
exec "$python_bin" -m engine "$@"
"#,
        ),
    ];
    if runtime_root != root {
        files.push(PlannedFile::new(
            runtime_root.join(".gitignore"),
            "state/\nartifacts/\ncache/\n",
        ));
    }
    Ok(files)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
