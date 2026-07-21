use clap::ValueEnum;
use serde::Serialize;

use crate::paths::{InstallScope, TargetRuntime};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeArg {
    Global,
    Project,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum TargetArg {
    Codex,
    Pi,
}

impl From<ScopeArg> for InstallScope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::Global => InstallScope::Global,
            ScopeArg::Project => InstallScope::Project,
        }
    }
}

impl From<TargetArg> for TargetRuntime {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Codex => TargetRuntime::Codex,
            TargetArg::Pi => TargetRuntime::Pi,
        }
    }
}
