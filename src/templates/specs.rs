use super::model::{TemplateKind, TemplateSpec};

pub(super) const TEMPLATE_SPECS: &[TemplateSpec] = &[
    TemplateSpec {
        name: "megara.toml",
        kind: TemplateKind::Config,
        relative_path: "megara.toml",
        description: "Megara harness configuration",
        content: include_str!("../../.agents/megara.toml"),
    },
    TemplateSpec {
        name: "README.md",
        kind: TemplateKind::Readme,
        relative_path: "README.md",
        description: "Megara harness overview",
        content: include_str!("../../.agents/README.md"),
    },
    TemplateSpec {
        name: "planning",
        kind: TemplateKind::Rule,
        relative_path: "rules/planning.md",
        description: "Planning boundary rules",
        content: include_str!("../../.agents/rules/planning.md"),
    },
    TemplateSpec {
        name: "deep-interview",
        kind: TemplateKind::Workflow,
        relative_path: "skills/deep-interview/SKILL.md",
        description: "Socratic requirements interview",
        content: include_str!("../../.agents/skills/deep-interview/SKILL.md"),
    },
    TemplateSpec {
        name: "ralplan",
        kind: TemplateKind::Workflow,
        relative_path: "skills/ralplan/SKILL.md",
        description: "Consensus planning workflow",
        content: include_str!("../../.agents/skills/ralplan/SKILL.md"),
    },
    TemplateSpec {
        name: "ultragoal",
        kind: TemplateKind::Workflow,
        relative_path: "skills/ultragoal/SKILL.md",
        description: "Durable goal execution workflow",
        content: include_str!("../../.agents/skills/ultragoal/SKILL.md"),
    },
    TemplateSpec {
        name: "team",
        kind: TemplateKind::Workflow,
        relative_path: "skills/team/SKILL.md",
        description: "Multi-agent lane coordination",
        content: include_str!("../../.agents/skills/team/SKILL.md"),
    },
    TemplateSpec {
        name: "deep-interview/auto-answer-uncertain",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/auto-answer-uncertain.md",
        description: "Deep Interview uncertain-answer fragment",
        content: include_str!(
            "../../.agents/skill-fragments/deep-interview/auto-answer-uncertain.md"
        ),
    },
    TemplateSpec {
        name: "deep-interview/auto-research-greenfield",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/auto-research-greenfield.md",
        description: "Deep Interview greenfield research fragment",
        content: include_str!(
            "../../.agents/skill-fragments/deep-interview/auto-research-greenfield.md"
        ),
    },
    TemplateSpec {
        name: "deep-interview/lateral-review-panel",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/deep-interview/lateral-review-panel.md",
        description: "Deep Interview lateral review fragment",
        content: include_str!(
            "../../.agents/skill-fragments/deep-interview/lateral-review-panel.md"
        ),
    },
    TemplateSpec {
        name: "ultragoal/ai-slop-cleaner",
        kind: TemplateKind::SkillFragment,
        relative_path: "skill-fragments/ultragoal/ai-slop-cleaner.md",
        description: "Ultragoal cleanup detector fragment",
        content: include_str!("../../.agents/skill-fragments/ultragoal/ai-slop-cleaner.md"),
    },
    TemplateSpec {
        name: "executor",
        kind: TemplateKind::Agent,
        relative_path: "agents/executor.toml",
        description: "Implementation agent",
        content: include_str!("../../.agents/agents/executor.toml"),
    },
    TemplateSpec {
        name: "planner",
        kind: TemplateKind::Agent,
        relative_path: "agents/planner.toml",
        description: "Planning agent",
        content: include_str!("../../.agents/agents/planner.toml"),
    },
    TemplateSpec {
        name: "architect",
        kind: TemplateKind::Agent,
        relative_path: "agents/architect.toml",
        description: "Architecture review agent",
        content: include_str!("../../.agents/agents/architect.toml"),
    },
    TemplateSpec {
        name: "critic",
        kind: TemplateKind::Agent,
        relative_path: "agents/critic.toml",
        description: "Plan critic agent",
        content: include_str!("../../.agents/agents/critic.toml"),
    },
];
