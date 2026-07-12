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
    pub(super) evidence_dir: PathBuf,
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
        let evidence_dir = dir.join("evidence");
        Ok(Self {
            brief_file: dir.join("brief.md"),
            goals_file: dir.join("goals.json"),
            ledger_file: dir.join("ledger.jsonl"),
            runtime_state_file: workflow_dir.join(format!("{safe_session}.json")),
            ralplan_state_file: workflow_base
                .join(RALPLAN)
                .join(format!("{safe_session}.json")),
            evidence_dir,
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
        "artifact_dir": paths.dir.display().to_string(),
        "evidence_dir": paths.evidence_dir.display().to_string(),
        "goals_path": paths.goals_file.display().to_string(),
        "ledger_path": paths.ledger_file.display().to_string(),
        "source": plan.source.clone(),
        "counts": goal_counts(&plan.goals),
        "active_goal_id": active_goal.map(|goal| goal.id.as_str()),
        "active_goal_title": active_goal.map(|goal| goal.title.as_str()),
        "updated_at": timestamp,
    });
    if phase == "goal_planning" {
        state["next"] = json!("run MEGARA_BIN=\"${MEGARA_BIN:-.agents/bin/megara}\"; \"$MEGARA_BIN\" ultragoal start-goal before mutating product files");
    } else if phase == "active" {
        state["next"] = json!("execute active goal and checkpoint with quality gate evidence");
    }
    write_json_atomic(&paths.runtime_state_file, &state)
}

pub(super) fn mark_source_transition_started(
    paths: &UltragoalPaths,
    plan: &UltragoalPlan,
    timestamp: &str,
) -> Result<()> {
    let Some(source_revision) = plan.source.as_ref().and_then(|source| {
        (source.kind == "ralplan")
            .then_some(source.ralplan_plan_sha256.as_deref())
            .flatten()
    }) else {
        return Ok(());
    };
    let Some(mut ralplan_state) = fs::read_to_string(&paths.ralplan_state_file)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
    else {
        return Ok(());
    };
    let Some(transition) = ralplan_state.get("transition") else {
        return Ok(());
    };
    if transition.get("target").and_then(Value::as_str) != Some(WORKFLOW)
        || transition.get("status").and_then(Value::as_str) != Some("starting")
        || transition.get("artifact_revision").and_then(Value::as_str) != Some(source_revision)
    {
        return Ok(());
    }
    let transition_id = transition
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if transition_id.is_empty() {
        return Ok(());
    }

    ralplan_state["transition"]["status"] = json!("started");
    ralplan_state["transition"]["target_started_at"] = json!(timestamp);
    ralplan_state["updated_at"] = json!(timestamp);
    write_json_atomic(&paths.ralplan_state_file, &ralplan_state)?;

    if let Some(mut runtime_state) = fs::read_to_string(&paths.runtime_state_file)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
    {
        runtime_state["source_transition_id"] = json!(transition_id);
        runtime_state["source_workflow"] = json!(RALPLAN);
        runtime_state["source_plan_sha256"] = json!(source_revision);
        write_json_atomic(&paths.runtime_state_file, &runtime_state)?;
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    let mut content = serde_json::to_string_pretty(value)?;
    content.push('\n');
    write_text_atomic(path, &content)
}
