use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct HarnessTemplate {
    pub name: &'static str,
    pub kind: TemplateKind,
    pub content: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateKind {
    Workflow,
    Agent,
}

#[derive(Clone, Debug)]
pub struct TemplateRegistry {
    workflows: Vec<HarnessTemplate>,
    agents: Vec<HarnessTemplate>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self {
            workflows: vec![
                HarnessTemplate {
                    name: "deep-interview",
                    kind: TemplateKind::Workflow,
                    content: DEEP_INTERVIEW,
                },
                HarnessTemplate {
                    name: "ralplan",
                    kind: TemplateKind::Workflow,
                    content: RALPLAN,
                },
                HarnessTemplate {
                    name: "ultragoal",
                    kind: TemplateKind::Workflow,
                    content: ULTRAGOAL,
                },
                HarnessTemplate {
                    name: "team",
                    kind: TemplateKind::Workflow,
                    content: TEAM,
                },
            ],
            agents: vec![
                HarnessTemplate {
                    name: "executor",
                    kind: TemplateKind::Agent,
                    content: EXECUTOR,
                },
                HarnessTemplate {
                    name: "planner",
                    kind: TemplateKind::Agent,
                    content: PLANNER,
                },
                HarnessTemplate {
                    name: "architect",
                    kind: TemplateKind::Agent,
                    content: ARCHITECT,
                },
                HarnessTemplate {
                    name: "critic",
                    kind: TemplateKind::Agent,
                    content: CRITIC,
                },
            ],
        }
    }
}

impl TemplateRegistry {
    pub fn workflows(&self) -> &[HarnessTemplate] {
        &self.workflows
    }

    pub fn agents(&self) -> &[HarnessTemplate] {
        &self.agents
    }

    pub fn template_names(&self) -> Vec<&'static str> {
        self.workflows
            .iter()
            .chain(self.agents.iter())
            .map(|template| template.name)
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&HarnessTemplate> {
        self.workflows
            .iter()
            .chain(self.agents.iter())
            .find(|template| template.name == name)
    }
}

const DEEP_INTERVIEW: &str = r#"# deep-interview

Use this workflow when the request is broad, ambiguous, or under-specified.

1. Restate the user's goal in concrete terms.
2. Identify assumptions and unresolved ambiguity.
3. Ask only questions that materially change the plan.
4. Convert answers into success criteria and constraints.
5. Stop when the next agent can act without guessing.
"#;

const RALPLAN: &str = r#"# ralplan

Use this workflow to turn a clarified goal into an implementation-ready plan.

1. Describe the intended outcome.
2. Identify affected surfaces and boundaries.
3. Break work into ordered tasks with verification for each task.
4. Record tradeoffs and defaults.
5. Produce a concise plan that can be executed directly.
"#;

const ULTRAGOAL: &str = r#"# ultragoal

Use this workflow for persistent execution toward a concrete goal.

1. Define the goal and done criteria.
2. Work in small increments.
3. Verify each increment before moving on.
4. Surface blockers with evidence.
5. Continue until the goal is complete or explicitly blocked.
"#;

const TEAM: &str = r#"# team

Use this workflow when multiple specialist agents should collaborate.

1. Split the goal into roles and responsibilities.
2. Assign planner, architect, executor, and critic work where useful.
3. Keep one owner for final integration.
4. Require critique before final delivery.
"#;

const EXECUTOR: &str = r#"# executor

Primary responsibility: make focused code changes that satisfy an accepted plan.

- Prefer simple implementation over speculative abstraction.
- Keep changes scoped to the task.
- Run the smallest meaningful verification before handing off.
"#;

const PLANNER: &str = r#"# planner

Primary responsibility: clarify goals and convert them into ordered work.

- Identify assumptions and dependencies.
- Define acceptance criteria.
- Keep plans short enough to execute.
"#;

const ARCHITECT: &str = r#"# architect

Primary responsibility: protect system boundaries and long-term shape.

- Name the affected modules and contracts.
- Prefer adapters over vendor lock-in.
- Call out irreversible decisions.
"#;

const CRITIC: &str = r#"# critic

Primary responsibility: review proposed or completed work for defects.

- Lead with concrete risks.
- Check missing tests and behavioral regressions.
- Keep feedback actionable.
"#;
