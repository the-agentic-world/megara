use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
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
    SkillFragment,
    Agent,
}

#[derive(Clone, Debug)]
pub struct TemplateRegistry {
    files: Vec<HarnessTemplate>,
}

struct TemplateSpec {
    name: &'static str,
    kind: TemplateKind,
    relative_path: &'static str,
    description: &'static str,
    content: &'static str,
}

const TEMPLATE_SPECS: &[TemplateSpec] = &[
    TemplateSpec {
        name: "megara.toml",
        kind: TemplateKind::Config,
        relative_path: "megara.toml",
        description: "Megara harness configuration",
        content: include_str!("../.agents/megara.toml"),
    },
    TemplateSpec {
        name: "README.md",
        kind: TemplateKind::Readme,
        relative_path: "README.md",
        description: "Megara harness overview",
        content: include_str!("../.agents/README.md"),
    },
    TemplateSpec {
        name: "planning",
        kind: TemplateKind::Rule,
        relative_path: "rules/planning.md",
        description: "Planning boundary rules",
        content: include_str!("../.agents/rules/planning.md"),
    },
    TemplateSpec {
        name: "deep-interview",
        kind: TemplateKind::Workflow,
        relative_path: "skills/deep-interview/SKILL.md",
        description: "Socratic requirements interview",
        content: include_str!("../.agents/skills/deep-interview/SKILL.md"),
    },
    TemplateSpec {
        name: "ralplan",
        kind: TemplateKind::Workflow,
        relative_path: "skills/ralplan/SKILL.md",
        description: "Consensus planning workflow",
        content: include_str!("../.agents/skills/ralplan/SKILL.md"),
    },
    TemplateSpec {
        name: "ultragoal",
        kind: TemplateKind::Workflow,
        relative_path: "skills/ultragoal/SKILL.md",
        description: "Durable goal execution workflow",
        content: include_str!("../.agents/skills/ultragoal/SKILL.md"),
    },
    TemplateSpec {
        name: "team",
        kind: TemplateKind::Workflow,
        relative_path: "skills/team/SKILL.md",
        description: "Multi-agent lane coordination",
        content: include_str!("../.agents/skills/team/SKILL.md"),
    },
    TemplateSpec {
        name: "deep-interview/auto-answer-uncertain",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/auto-answer-uncertain.md",
        description: "Deep Interview uncertain-answer fragment",
        content: include_str!("../.agents/skill-fragments/deep-interview/auto-answer-uncertain.md"),
    },
    TemplateSpec {
        name: "deep-interview/auto-research-greenfield",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/auto-research-greenfield.md",
        description: "Deep Interview greenfield research fragment",
        content: include_str!(
            "../.agents/skill-fragments/deep-interview/auto-research-greenfield.md"
        ),
    },
    TemplateSpec {
        name: "deep-interview/lateral-review-panel",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/lateral-review-panel.md",
        description: "Deep Interview lateral review fragment",
        content: include_str!("../.agents/skill-fragments/deep-interview/lateral-review-panel.md"),
    },
    TemplateSpec {
        name: "ultragoal/ai-slop-cleaner",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/ultragoal/ai-slop-cleaner.md",
        description: "Ultragoal cleanup detector fragment",
        content: include_str!("../.agents/skill-fragments/ultragoal/ai-slop-cleaner.md"),
    },
    TemplateSpec {
        name: "executor",
        kind: TemplateKind::Agent,
        relative_path: "agents/executor.md",
        description: "Implementation agent",
        content: include_str!("../.agents/agents/executor.md"),
    },
    TemplateSpec {
        name: "planner",
        kind: TemplateKind::Agent,
        relative_path: "agents/planner.md",
        description: "Planning agent",
        content: include_str!("../.agents/agents/planner.md"),
    },
    TemplateSpec {
        name: "architect",
        kind: TemplateKind::Agent,
        relative_path: "agents/architect.md",
        description: "Architecture review agent",
        content: include_str!("../.agents/agents/architect.md"),
    },
    TemplateSpec {
        name: "critic",
        kind: TemplateKind::Agent,
        relative_path: "agents/critic.md",
        description: "Plan critic agent",
        content: include_str!("../.agents/agents/critic.md"),
    },
];

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self {
            files: TEMPLATE_SPECS
                .iter()
                .map(|spec| HarnessTemplate {
                    name: spec.name.to_string(),
                    kind: spec.kind,
                    relative_path: spec.relative_path.to_string(),
                    description: spec.description.to_string(),
                    content: spec.content.to_string(),
                })
                .collect(),
        }
    }
}

impl TemplateRegistry {
    pub fn from_ssot_root(root: &Path) -> Result<Self> {
        let files = TEMPLATE_SPECS
            .iter()
            .map(|spec| {
                let path = root.join(spec.relative_path);
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read SSOT file {}", path.display()))?;
                Ok(HarnessTemplate {
                    name: spec.name.to_string(),
                    kind: spec.kind,
                    relative_path: spec.relative_path.to_string(),
                    description: spec.description.to_string(),
                    content: strip_managed_marker(&content),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { files })
    }

    pub fn missing_paths(root: &Path) -> Vec<PathBuf> {
        TEMPLATE_SPECS
            .iter()
            .map(|spec| root.join(spec.relative_path))
            .filter(|path| !path.exists())
            .collect()
    }

    pub fn ssot_files(&self) -> &[HarnessTemplate] {
        &self.files
    }

    pub fn workflows(&self) -> Vec<&HarnessTemplate> {
        self.files
            .iter()
            .filter(|template| template.kind == TemplateKind::Workflow)
            .collect()
    }

    pub fn fragments(&self) -> Vec<&HarnessTemplate> {
        self.files
            .iter()
            .filter(|template| template.kind == TemplateKind::SkillFragment)
            .collect()
    }

    pub fn agents(&self) -> Vec<&HarnessTemplate> {
        self.files
            .iter()
            .filter(|template| template.kind == TemplateKind::Agent)
            .collect()
    }

    pub fn template_names(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|template| template.name.clone())
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&HarnessTemplate> {
        self.files
            .iter()
            .find(|template| template.name == name || template.relative_path == name)
    }
}
