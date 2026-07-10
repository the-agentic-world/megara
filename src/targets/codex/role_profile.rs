#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CodexRoleProfile {
    pub(crate) model: &'static str,
    pub(crate) reasoning_effort: &'static str,
}

pub(crate) fn role_profile(role: &str) -> Option<CodexRoleProfile> {
    let profile = match role.trim().to_ascii_lowercase().as_str() {
        "executor" | "planner" => CodexRoleProfile {
            model: "gpt-5.6-terra",
            reasoning_effort: "high",
        },
        "architect" => CodexRoleProfile {
            model: "gpt-5.6-sol",
            reasoning_effort: "xhigh",
        },
        "critic" | "contrarian" => CodexRoleProfile {
            model: "gpt-5.6-sol",
            reasoning_effort: "high",
        },
        "researcher" => CodexRoleProfile {
            model: "gpt-5.6-terra",
            reasoning_effort: "medium",
        },
        "simplifier" => CodexRoleProfile {
            model: "gpt-5.6-luna",
            reasoning_effort: "high",
        },
        _ => return None,
    };
    Some(profile)
}
