use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::{cli::ScopeArg, paths::home_dir};

pub(super) use super::util::{
    ensure_trailing_newline, safe_part, scope_label, sha256_hex, timestamp, write_text_atomic,
};
use super::{flow::goal_counts, model::UltragoalPlan, RALPLAN, WORKFLOW};

#[derive(Debug)]
pub(super) struct UltragoalPaths {
    pub(super) dir: PathBuf,
    pub(super) brief_file: PathBuf,
    pub(super) goals_file: PathBuf,
    pub(super) ledger_file: PathBuf,
    pub(super) runtime_state_file: PathBuf,
    pub(super) ralplan_state_file: PathBuf,
}

impl UltragoalPaths {
    pub(super) fn resolve(scope: ScopeArg, session_id: &str) -> Result<Self> {
        let runtime_root = match scope {
            ScopeArg::Project => env::current_dir()
                .context("failed to read current directory")?
                .join(".megara"),
            ScopeArg::Global => home_dir()?.join(".megara"),
        };
        let workflow_base = runtime_root.join("state").join("workflows");
        let artifact_base = runtime_root.join("artifacts");
        let safe_session = safe_part(session_id);
        let workflow_dir = workflow_base.join(WORKFLOW);
        let dir = artifact_base.join(WORKFLOW).join(&safe_session);
        Ok(Self {
            brief_file: dir.join("brief.md"),
            goals_file: dir.join("goals.json"),
            ledger_file: dir.join("ledger.jsonl"),
            runtime_state_file: workflow_dir.join(format!("{safe_session}.json")),
            ralplan_state_file: workflow_base
                .join(RALPLAN)
                .join(format!("{safe_session}.json")),
            dir,
        })
    }
}

pub(super) fn read_plan(paths: &UltragoalPaths) -> Result<Option<UltragoalPlan>> {
    if !paths.goals_file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&paths.goals_file)
        .with_context(|| format!("failed to read {}", paths.goals_file.display()))?;
    let plan = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", paths.goals_file.display()))?;
    Ok(Some(plan))
}

pub(super) fn read_plan_required(paths: &UltragoalPaths) -> Result<UltragoalPlan> {
    read_plan(paths)?.with_context(|| {
        format!(
            "ultragoal session is missing at {}; run `MEGARA_BIN=\"${{MEGARA_BIN:-.agents/bin/megara}}\"; \"$MEGARA_BIN\" ultragoal create-goals` first",
            paths.dir.display()
        )
    })
}

pub(super) fn write_plan(paths: &UltragoalPaths, plan: &UltragoalPlan) -> Result<()> {
    fs::create_dir_all(&paths.dir)?;
    let mut content = serde_json::to_string_pretty(plan)?;
    content.push('\n');
    write_text_atomic(&paths.goals_file, &content)
}

pub(super) fn append_ledger(paths: &UltragoalPaths, entry: &Value) -> Result<()> {
    if let Some(parent) = paths.ledger_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.ledger_file)?;
    serde_json::to_writer(&mut file, entry)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub(super) fn write_runtime_state(
    paths: &UltragoalPaths,
    plan: &UltragoalPlan,
    phase: &str,
    active: bool,
    timestamp: &str,
) -> Result<()> {
    let active_goal = plan.goals.iter().find(|goal| goal.status == "active");
    let mut state = json!({
        "version": 1,
        "skill": WORKFLOW,
        "session_id": plan.session_id.clone(),
        "active": active,
        "phase": phase,
        "status": phase,
        "brief_path": plan.brief_path.clone(),
        "brief_sha256": plan.brief_sha256.clone(),
        "goals_path": paths.goals_file.display().to_string(),
        "ledger_path": paths.ledger_file.display().to_string(),
        "source": plan.source.clone(),
        "counts": goal_counts(&plan.goals),
        "active_goal_id": active_goal.map(|goal| goal.id.as_str()),
        "active_goal_title": active_goal.map(|goal| goal.title.as_str()),
        "updated_at": timestamp,
    });
    if phase == "goal_planning" {
        state["next"] = json!("run MEGARA_BIN=\"${MEGARA_BIN:-.agents/bin/megara}\"; \"$MEGARA_BIN\" ultragoal complete-goals before mutating product files");
    } else if phase == "active" {
        state["next"] = json!("execute active goal and checkpoint with quality gate evidence");
    }
    write_json_atomic(&paths.runtime_state_file, &state)
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    let mut content = serde_json::to_string_pretty(value)?;
    content.push('\n');
    write_text_atomic(path, &content)
}
