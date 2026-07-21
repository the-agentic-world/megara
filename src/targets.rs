#[path = "targets/codex.rs"]
pub mod codex;
#[path = "targets/pi.rs"]
pub mod pi;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct TargetInfo {
    pub name: &'static str,
    pub status: &'static str,
}

pub fn supported_targets() -> Vec<TargetInfo> {
    vec![
        TargetInfo {
            name: "codex",
            status: "supported",
        },
        TargetInfo {
            name: "pi",
            status: "supported",
        },
    ]
}
