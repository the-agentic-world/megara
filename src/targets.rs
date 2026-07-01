pub mod codex;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct TargetInfo {
    pub name: &'static str,
    pub status: &'static str,
}

pub fn supported_targets() -> Vec<TargetInfo> {
    vec![TargetInfo {
        name: "codex",
        status: "supported",
    }]
}
