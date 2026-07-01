use std::{env, fmt, path::PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallScope {
    Global,
    Project,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetRuntime {
    Codex,
}

impl fmt::Display for InstallScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallScope::Global => write!(f, "global"),
            InstallScope::Project => write!(f, "project"),
        }
    }
}

impl fmt::Display for TargetRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetRuntime::Codex => write!(f, "codex"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallPaths {
    pub ssot_root: PathBuf,
    pub target_root: PathBuf,
}

impl InstallPaths {
    pub fn resolve(scope: InstallScope, target: TargetRuntime) -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        match scope {
            InstallScope::Project => Ok(Self {
                ssot_root: cwd.join(".agents"),
                target_root: target.project_root(cwd),
            }),
            InstallScope::Global => {
                let home = home_dir()?;
                Ok(Self {
                    ssot_root: home.join(".megara"),
                    target_root: target.global_root(home),
                })
            }
        }
    }
}

impl TargetRuntime {
    fn project_root(self, cwd: PathBuf) -> PathBuf {
        match self {
            TargetRuntime::Codex => cwd.join(".codex"),
        }
    }

    fn global_root(self, home: PathBuf) -> PathBuf {
        match self {
            TargetRuntime::Codex => home.join(".codex"),
        }
    }
}

pub fn home_dir() -> Result<PathBuf> {
    match env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => bail!("HOME is not set"),
    }
}
