use serde::Serialize;

use crate::installer::strip_managed_marker;

#[derive(Clone, Debug, Serialize)]
pub struct HarnessTemplate {
    pub name: String,
    pub kind: TemplateKind,
    pub relative_path: String,
    pub description: String,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateKind {
    Config,
    Readme,
    Rule,
    Workflow,
    Skill,
    SkillFragment,
    Tool,
    ToolSupport,
    Agent,
}

pub(super) struct TemplateSpec {
    pub(super) name: &'static str,
    pub(super) kind: TemplateKind,
    pub(super) relative_path: &'static str,
    pub(super) description: &'static str,
    pub(super) content: &'static str,
}

impl TemplateSpec {
    pub(super) fn to_template(&self) -> HarnessTemplate {
        HarnessTemplate {
            name: self.name.to_string(),
            kind: self.kind,
            relative_path: self.relative_path.to_string(),
            description: self.description.to_string(),
            content: strip_managed_marker(self.content),
        }
    }
}
