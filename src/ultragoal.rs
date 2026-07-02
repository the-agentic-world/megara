use std::{
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    cli::{
        ScopeArg, UltragoalArgs, UltragoalCheckpointArgs, UltragoalCommands,
        UltragoalCompleteGoalsArgs, UltragoalCreateGoalsArgs, UltragoalGoalStatusArg,
        UltragoalStatusArgs, UltragoalSteerArgs, UltragoalSteerKindArg,
    },
    paths::home_dir,
};

const WORKFLOW: &str = "ultragoal";
const RALPLAN: &str = "ralplan";

#[derive(Debug)]
struct UltragoalPaths {
    dir: PathBuf,
    brief_file: PathBuf,
    goals_file: PathBuf,
    ledger_file: PathBuf,
    runtime_state_file: PathBuf,
    ralplan_state_file: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct UltragoalPlan {
    version: u32,
    scope: String,
    session_id: String,
    brief_path: String,
    brief_sha256: String,
    #[serde(default)]
    source: Option<UltragoalSource>,
    goals: Vec<UltragoalGoal>,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UltragoalSource {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ralplan_plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ralplan_plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ralplan_plan_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_spec_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_spec_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UltragoalGoal {
    id: String,
    title: String,
    objective: String,
    status: String,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    evidence: Option<String>,
    completion_receipt: Option<CompletionReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompletionReceipt {
    schema_version: u32,
    receipt_id: String,
    goal_id: String,
    verified_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brief_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_plan_sha256: Option<String>,
    quality_gate_sha256: String,
    evidence_sha256: String,
}

#[derive(Debug, Serialize)]
struct StatusReport<'a> {
    session_id: &'a str,
    path: &'a Path,
    state: &'static str,
    counts: GoalCounts,
    active_goal: Option<&'a UltragoalGoal>,
    goals: &'a [UltragoalGoal],
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct GoalCounts {
    pending: usize,
    active: usize,
    complete: usize,
    failed: usize,
    blocked: usize,
    review_blocked: usize,
    superseded: usize,
}

struct ParsedGoal {
    title: String,
    objective: String,
}

struct BriefSource {
    content: String,
    source: UltragoalSource,
}

pub fn run(args: UltragoalArgs) -> Result<()> {
    let paths = UltragoalPaths::resolve(args.scope, &args.session_id)?;
    match args.command {
        UltragoalCommands::Status(command) => status(&paths, &args.session_id, command),
        UltragoalCommands::CreateGoals(command) => {
            create_goals(&paths, args.scope, &args.session_id, command)
        }
        UltragoalCommands::CompleteGoals(command) => complete_goals(&paths, command),
        UltragoalCommands::Checkpoint(command) => checkpoint(&paths, command),
        UltragoalCommands::Steer(command) => steer(&paths, command),
    }
}

impl UltragoalPaths {
    fn resolve(scope: ScopeArg, session_id: &str) -> Result<Self> {
        let ssot_root = match scope {
            ScopeArg::Project => env::current_dir()
                .context("failed to read current directory")?
                .join(".agents"),
            ScopeArg::Global => home_dir()?.join(".megara"),
        };
        let workflow_base = ssot_root.join("state").join("workflows");
        let safe_session = safe_part(session_id);
        let workflow_dir = workflow_base.join(WORKFLOW);
        let dir = workflow_dir.join(&safe_session);
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

impl UltragoalGoalStatusArg {
    fn as_status(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::ReviewBlocked => "review_blocked",
            Self::Superseded => "superseded",
        }
    }
}

fn status(paths: &UltragoalPaths, session_id: &str, args: UltragoalStatusArgs) -> Result<()> {
    let Some(plan) = read_plan(paths)? else {
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "session_id": session_id,
                    "path": paths.dir.display().to_string(),
                    "state": "missing",
                    "counts": GoalCounts::default(),
                    "active_goal": Value::Null,
                    "goals": [],
                }))?
            );
        } else {
            println!(
                "ultragoal session {session_id}: no goals created ({})",
                paths.dir.display()
            );
        }
        return Ok(());
    };

    let report = StatusReport {
        session_id: &plan.session_id,
        path: &paths.dir,
        state: "ready",
        counts: goal_counts(&plan.goals),
        active_goal: plan.goals.iter().find(|goal| goal.status == "active"),
        goals: &plan.goals,
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let active = report
            .active_goal
            .map(|goal| format!("{} - {}", goal.id, goal.title))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "ultragoal session {}: active={}, pending={}, complete={}, failed={}, blocked={}",
            report.session_id,
            active,
            report.counts.pending,
            report.counts.complete,
            report.counts.failed,
            report.counts.blocked + report.counts.review_blocked
        );
    }
    Ok(())
}

fn create_goals(
    paths: &UltragoalPaths,
    scope: ScopeArg,
    session_id: &str,
    args: UltragoalCreateGoalsArgs,
) -> Result<()> {
    if paths.goals_file.exists() && !args.force {
        bail!(
            "ultragoal session already exists at {}; pass --force to recreate it",
            paths.goals_file.display()
        );
    }
    if paths.dir.exists() && args.force {
        fs::remove_dir_all(&paths.dir)
            .with_context(|| format!("failed to reset {}", paths.dir.display()))?;
    }
    let brief_source = read_brief_source(paths, &args)?;
    let brief = brief_source.content;
    if brief.trim().is_empty() {
        bail!("ultragoal brief is empty");
    }
    let parsed_goals = parse_goals(&brief)?;
    let timestamp = timestamp();
    let goals = parsed_goals
        .into_iter()
        .enumerate()
        .map(|(index, parsed)| UltragoalGoal {
            id: format!("G{:03}", index + 1),
            title: parsed.title,
            objective: parsed.objective,
            status: "pending".to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            started_at: None,
            completed_at: None,
            evidence: None,
            completion_receipt: None,
        })
        .collect::<Vec<_>>();

    fs::create_dir_all(&paths.dir)?;
    write_text_atomic(&paths.brief_file, ensure_trailing_newline(&brief).as_str())?;
    let brief_sha256 = sha256_hex(brief.as_bytes());
    let plan = UltragoalPlan {
        version: 1,
        scope: scope_label(scope).to_string(),
        session_id: session_id.to_string(),
        brief_path: paths.brief_file.display().to_string(),
        brief_sha256,
        source: Some(brief_source.source),
        goals,
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
    };
    write_plan(paths, &plan)?;
    write_runtime_state(paths, &plan, "goal_planning", true, &timestamp)?;
    append_ledger(
        paths,
        &json!({
            "timestamp": timestamp,
            "event": "goals_created",
            "session_id": session_id,
            "source": plan.source.clone(),
            "goal_count": plan.goals.len(),
            "brief_path": paths.brief_file.display().to_string(),
            "goals_path": paths.goals_file.display().to_string(),
        }),
    )?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!(
            "ultragoal session {}: created {} goal(s)",
            plan.session_id,
            plan.goals.len()
        );
    }
    Ok(())
}

fn complete_goals(paths: &UltragoalPaths, args: UltragoalCompleteGoalsArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    let Some(index) = next_goal_index(&plan.goals, args.retry_failed) else {
        write_runtime_state(paths, &plan, "complete", false, &timestamp)?;
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "state": "complete",
                    "session_id": plan.session_id,
                    "next_goal": Value::Null,
                    "counts": goal_counts(&plan.goals),
                }))?
            );
        } else {
            println!("ultragoal session {}: all goals complete", plan.session_id);
        }
        return Ok(());
    };

    let goal = &mut plan.goals[index];
    let was_active = goal.status == "active";
    if !was_active {
        goal.status = "active".to_string();
        goal.started_at.get_or_insert_with(|| timestamp.clone());
        goal.updated_at = timestamp.clone();
        plan.updated_at = timestamp.clone();
        append_ledger(
            paths,
            &json!({
                "timestamp": timestamp,
                "event": "goal_started",
                "session_id": plan.session_id,
                "goal_id": goal.id,
                "title": goal.title,
            }),
        )?;
        write_plan(paths, &plan)?;
    }
    write_runtime_state(paths, &plan, "active", true, &timestamp)?;
    let goal = &plan.goals[index];
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "state": if was_active { "resumed" } else { "started" },
                "session_id": plan.session_id,
                "next_goal": goal,
                "counts": goal_counts(&plan.goals),
            }))?
        );
    } else {
        println!(
            "ultragoal next-action=execute-goal goal-id={}\ntitle={}\nobjective={}\ncheckpoint requires=architectReview:CLEAR+APPROVE,executorQa:passed,iteration:passed",
            goal.id, goal.title, goal.objective
        );
    }
    Ok(())
}

fn checkpoint(paths: &UltragoalPaths, args: UltragoalCheckpointArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    let index = plan
        .goals
        .iter()
        .position(|goal| goal.id == args.goal_id)
        .with_context(|| format!("unknown ultragoal goal id: {}", args.goal_id))?;
    let status = args.status.as_status();
    if args.evidence.trim().is_empty() {
        bail!("checkpoint evidence is required");
    }
    if status == "complete" && plan.goals[index].status != "active" {
        bail!(
            "complete checkpoints require an active goal; run `megara ultragoal complete-goals` first"
        );
    }

    let receipt = if status == "complete" {
        let raw = args
            .quality_gate_json
            .as_deref()
            .context("--quality-gate-json is required for complete checkpoints")?;
        let quality_gate = read_quality_gate(raw)?;
        let artifact_root = env::current_dir().context("failed to read current directory")?;
        validate_quality_gate(&quality_gate, &artifact_root)?;
        Some(completion_receipt(
            &plan,
            &plan.goals[index].id,
            &args.evidence,
            &quality_gate,
            &timestamp,
        )?)
    } else {
        None
    };

    {
        let goal = &mut plan.goals[index];
        goal.status = status.to_string();
        goal.evidence = Some(args.evidence.clone());
        goal.updated_at = timestamp.clone();
        if status == "active" {
            goal.started_at.get_or_insert_with(|| timestamp.clone());
        }
        if status == "complete" {
            goal.completed_at = Some(timestamp.clone());
            goal.completion_receipt = receipt.clone();
        }
    }

    let next_started = if status == "complete" {
        start_next_pending_goal(&mut plan, &timestamp)
    } else {
        None
    };
    plan.updated_at = timestamp.clone();
    write_plan(paths, &plan)?;
    let (runtime_phase, runtime_active) = runtime_phase_for_plan(&plan);
    write_runtime_state(paths, &plan, runtime_phase, runtime_active, &timestamp)?;

    append_ledger(
        paths,
        &json!({
            "timestamp": timestamp,
            "event": "goal_checkpointed",
            "session_id": plan.session_id,
            "goal_id": args.goal_id,
            "status": status,
            "evidence": args.evidence,
            "completion_receipt": receipt,
            "next_goal_started": next_started.clone(),
        }),
    )?;

    if let Some(goal) = &next_started {
        append_ledger(
            paths,
            &json!({
                "timestamp": timestamp,
                "event": "goal_started",
                "session_id": plan.session_id,
                "goal_id": goal["id"],
                "title": goal["title"],
            }),
        )?;
    }

    let goal = &plan.goals[index];
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "session_id": plan.session_id,
                "goal": goal,
                "next_goal_started": next_started,
                "counts": goal_counts(&plan.goals),
            }))?
        );
    } else if let Some(next) = next_started {
        println!(
            "ultragoal checkpoint recorded for {} ({status}); next active goal: {} - {}",
            goal.id, next["id"], next["title"]
        );
    } else {
        println!("ultragoal checkpoint recorded for {} ({status})", goal.id);
    }
    Ok(())
}

fn steer(paths: &UltragoalPaths, args: UltragoalSteerArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    match args.kind {
        UltragoalSteerKindArg::AddSubgoal => {
            let title = args
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("--title is required for --kind add-subgoal")?;
            let objective = args
                .objective
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("--objective is required for --kind add-subgoal")?;
            let goal = UltragoalGoal {
                id: format!("G{:03}", plan.goals.len() + 1),
                title: title.to_string(),
                objective: objective.to_string(),
                status: "pending".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
                started_at: None,
                completed_at: None,
                evidence: args.evidence.clone(),
                completion_receipt: None,
            };
            plan.goals.push(goal.clone());
            plan.updated_at = timestamp.clone();
            write_plan(paths, &plan)?;
            let (runtime_phase, runtime_active) = runtime_phase_for_plan(&plan);
            write_runtime_state(paths, &plan, runtime_phase, runtime_active, &timestamp)?;
            append_ledger(
                paths,
                &json!({
                    "timestamp": timestamp,
                    "event": "goal_added",
                    "session_id": plan.session_id,
                    "goal": goal.clone(),
                    "rationale": args.rationale,
                    "evidence": args.evidence,
                }),
            )?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&goal)?);
            } else {
                println!("ultragoal added {} - {}", goal.id, goal.title);
            }
        }
        UltragoalSteerKindArg::AnnotateLedger => {
            let evidence = args
                .evidence
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("--evidence is required for --kind annotate-ledger")?;
            append_ledger(
                paths,
                &json!({
                    "timestamp": timestamp,
                    "event": "ledger_annotation",
                    "session_id": plan.session_id,
                    "rationale": args.rationale,
                    "evidence": evidence,
                }),
            )?;
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "event": "ledger_annotation",
                        "session_id": plan.session_id,
                        "evidence": evidence,
                    }))?
                );
            } else {
                println!("ultragoal ledger annotation recorded");
            }
        }
    }
    Ok(())
}

fn read_brief(
    brief: Option<String>,
    brief_file: Option<&Path>,
    from_stdin: bool,
) -> Result<String> {
    if let Some(brief) = brief {
        return Ok(brief);
    }
    if let Some(path) = brief_file {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read brief file: {}", path.display()));
    }
    if from_stdin {
        let mut value = String::new();
        io::stdin().read_to_string(&mut value)?;
        return Ok(value);
    }
    bail!("provide --brief, --brief-file, or --from-stdin")
}

fn read_brief_source(
    paths: &UltragoalPaths,
    args: &UltragoalCreateGoalsArgs,
) -> Result<BriefSource> {
    let has_direct_source = args.brief.is_some() || args.brief_file.is_some() || args.from_stdin;
    if has_direct_source {
        let content = read_brief(
            args.brief.clone(),
            args.brief_file.as_deref(),
            args.from_stdin,
        )?;
        if args.allow_direct {
            return Ok(BriefSource {
                content,
                source: direct_source(),
            });
        }

        if let Ok(approved) = approved_ralplan_handoff(paths) {
            if sha256_hex(content.as_bytes())
                == approved
                    .source
                    .ralplan_plan_sha256
                    .as_deref()
                    .unwrap_or_default()
            {
                return Ok(approved);
            }
        }

        bail!(
            "direct ultragoal brief is blocked; omit brief flags to consume an approved ralplan handoff, or pass --allow-direct for an explicit direct run"
        );
    }

    approved_ralplan_handoff(paths)
}

fn approved_ralplan_handoff(paths: &UltragoalPaths) -> Result<BriefSource> {
    let state = fs::read_to_string(&paths.ralplan_state_file).with_context(|| {
        format!(
            "approved ralplan handoff is missing at {}; approve a ralplan plan first or pass --allow-direct",
            paths.ralplan_state_file.display()
        )
    })?;
    let state = serde_json::from_str::<Value>(&state).with_context(|| {
        format!(
            "failed to parse ralplan handoff state: {}",
            paths.ralplan_state_file.display()
        )
    })?;
    if state.get("skill").and_then(Value::as_str) != Some(RALPLAN)
        || state.get("approval_status").and_then(Value::as_str) != Some("approved")
        || state.get("approved_handoff_target").and_then(Value::as_str) != Some(WORKFLOW)
    {
        bail!(
            "approved ralplan handoff is not ready for ultragoal; approve the pending plan for ultragoal first"
        );
    }

    let plan_path = state
        .get("plan_path")
        .and_then(Value::as_str)
        .context("approved ralplan handoff is missing plan_path")?;
    let expected_sha256 = state
        .get("approved_plan_sha256")
        .or_else(|| state.get("plan_sha256"))
        .and_then(Value::as_str)
        .context("approved ralplan handoff is missing approved_plan_sha256")?;
    let content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read approved ralplan plan: {plan_path}"))?;
    let actual_sha256 = sha256_hex(content.as_bytes());
    if actual_sha256 != expected_sha256 {
        bail!(
            "approved ralplan plan hash mismatch: expected {expected_sha256}, got {actual_sha256}"
        );
    }

    Ok(BriefSource {
        content,
        source: UltragoalSource {
            kind: "ralplan".to_string(),
            ralplan_plan_id: state
                .get("approved_plan_id")
                .or_else(|| state.get("plan_id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            ralplan_plan_path: Some(plan_path.to_string()),
            ralplan_plan_sha256: Some(expected_sha256.to_string()),
            input_spec_path: state
                .get("input_spec_path")
                .and_then(Value::as_str)
                .map(str::to_string),
            input_spec_sha256: state
                .get("input_spec_sha256")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
    })
}

fn direct_source() -> UltragoalSource {
    UltragoalSource {
        kind: "direct".to_string(),
        ralplan_plan_id: None,
        ralplan_plan_path: None,
        ralplan_plan_sha256: None,
        input_spec_path: None,
        input_spec_sha256: None,
    }
}

fn parse_goals(brief: &str) -> Result<Vec<ParsedGoal>> {
    let mut sections = Vec::<(String, Vec<String>)>::new();
    for line in brief.lines() {
        if let Some(title) = goal_marker(line) {
            sections.push((title, Vec::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push(line.to_string());
        }
    }

    if sections.is_empty() {
        let objective = brief.trim();
        if objective.is_empty() {
            bail!("ultragoal brief has no goal content");
        }
        return Ok(vec![ParsedGoal {
            title: title_from_objective(objective),
            objective: objective.to_string(),
        }]);
    }

    sections
        .into_iter()
        .enumerate()
        .map(|(index, (title, body_lines))| {
            let body = body_lines.join("\n").trim().to_string();
            let title = if title.trim().is_empty() {
                title_from_objective(&body)
            } else {
                title.trim().to_string()
            };
            let objective = if body.is_empty() { title.clone() } else { body };
            if title.trim().is_empty() || objective.trim().is_empty() {
                bail!("ultragoal @goal block {} is empty", index + 1);
            }
            Ok(ParsedGoal { title, objective })
        })
        .collect()
}

fn goal_marker(line: &str) -> Option<String> {
    let rest = line.strip_prefix("@goal")?;
    if rest.is_empty() {
        return Some(String::new());
    }
    let mut chars = rest.chars();
    match chars.next()? {
        ':' => Some(chars.as_str().trim().to_string()),
        ' ' | '\t' => Some(chars.as_str().trim().to_string()),
        _ => None,
    }
}

fn title_from_objective(objective: &str) -> String {
    let first = objective
        .lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .unwrap_or("Complete ultragoal brief");
    first.chars().take(96).collect()
}

fn next_goal_index(goals: &[UltragoalGoal], retry_failed: bool) -> Option<usize> {
    goals
        .iter()
        .position(|goal| goal.status == "active")
        .or_else(|| goals.iter().position(|goal| goal.status == "pending"))
        .or_else(|| retry_failed.then(|| goals.iter().position(|goal| goal.status == "failed"))?)
}

fn start_next_pending_goal(plan: &mut UltragoalPlan, timestamp: &str) -> Option<Value> {
    if plan.goals.iter().any(|goal| goal.status == "active") {
        return None;
    }
    let index = plan
        .goals
        .iter()
        .position(|goal| goal.status == "pending")?;
    let goal = &mut plan.goals[index];
    goal.status = "active".to_string();
    goal.started_at.get_or_insert_with(|| timestamp.to_string());
    goal.updated_at = timestamp.to_string();
    Some(json!({
        "id": goal.id,
        "title": goal.title,
        "objective": goal.objective,
    }))
}

fn goal_counts(goals: &[UltragoalGoal]) -> GoalCounts {
    goals
        .iter()
        .fold(GoalCounts::default(), |mut counts, goal| {
            match goal.status.as_str() {
                "pending" => counts.pending += 1,
                "active" => counts.active += 1,
                "complete" => counts.complete += 1,
                "failed" => counts.failed += 1,
                "blocked" => counts.blocked += 1,
                "review_blocked" => counts.review_blocked += 1,
                "superseded" => counts.superseded += 1,
                _ => {}
            }
            counts
        })
}

fn read_plan(paths: &UltragoalPaths) -> Result<Option<UltragoalPlan>> {
    if !paths.goals_file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&paths.goals_file)
        .with_context(|| format!("failed to read {}", paths.goals_file.display()))?;
    let plan = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", paths.goals_file.display()))?;
    Ok(Some(plan))
}

fn read_plan_required(paths: &UltragoalPaths) -> Result<UltragoalPlan> {
    read_plan(paths)?.with_context(|| {
        format!(
            "ultragoal session is missing at {}; run `megara ultragoal create-goals` first",
            paths.dir.display()
        )
    })
}

fn write_plan(paths: &UltragoalPaths, plan: &UltragoalPlan) -> Result<()> {
    fs::create_dir_all(&paths.dir)?;
    let mut content = serde_json::to_string_pretty(plan)?;
    content.push('\n');
    write_text_atomic(&paths.goals_file, &content)
}

fn append_ledger(paths: &UltragoalPaths, entry: &Value) -> Result<()> {
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

fn runtime_phase_for_plan(plan: &UltragoalPlan) -> (&'static str, bool) {
    let counts = goal_counts(&plan.goals);
    if counts.active > 0 {
        ("active", true)
    } else if counts.pending > 0 {
        ("goal_planning", true)
    } else if counts.blocked + counts.review_blocked > 0 {
        ("blocked", true)
    } else if counts.failed > 0 {
        ("failed", true)
    } else {
        ("complete", false)
    }
}

fn write_runtime_state(
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
        state["next"] = json!("run megara ultragoal complete-goals before mutating product files");
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

fn read_quality_gate(value_or_path: &str) -> Result<Value> {
    let trimmed = value_or_path.trim();
    let raw = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else {
        fs::read_to_string(value_or_path)
            .with_context(|| format!("failed to read quality gate json: {value_or_path}"))?
    };
    serde_json::from_str(&raw).context("failed to parse quality gate json")
}

fn validate_quality_gate(value: &Value, artifact_root: &Path) -> Result<()> {
    require_object(value, "quality gate")?;
    let architect = section(value, "architectReview")?;
    require_str_eq(architect, "recommendation", "APPROVE")?;
    require_str_eq(architect, "architectureStatus", "CLEAR")?;
    require_str_eq(architect, "productStatus", "CLEAR")?;
    require_str_eq(architect, "codeStatus", "CLEAR")?;
    require_substantive_str(architect, "evidence")?;
    validate_artifact_refs(architect, "reviewedFiles", artifact_root)?;
    require_empty_array(architect, "blockers")?;

    let qa = section(value, "executorQa")?;
    require_str_eq(qa, "status", "passed")?;
    require_str_eq(qa, "e2eStatus", "passed")?;
    require_str_eq(qa, "redTeamStatus", "passed")?;
    require_substantive_str(qa, "evidence")?;
    require_string_array(qa, "commands")?;
    validate_artifact_refs(qa, "artifactRefs", artifact_root)?;
    require_empty_array(qa, "blockers")?;

    let iteration = section(value, "iteration")?;
    require_str_eq(iteration, "status", "passed")?;
    if iteration.get("fullRerun").and_then(Value::as_bool) != Some(true) {
        bail!("quality gate iteration.fullRerun must be true");
    }
    require_substantive_str(iteration, "evidence")?;
    require_string_array(iteration, "commands")?;
    validate_artifact_refs(iteration, "artifactRefs", artifact_root)?;
    require_empty_array(iteration, "blockers")?;
    Ok(())
}

fn section<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    let section = value
        .get(key)
        .with_context(|| format!("quality gate missing {key}"))?;
    require_object(section, key)?;
    Ok(section)
}

fn require_object(value: &Value, label: &str) -> Result<()> {
    if value.as_object().is_none() {
        bail!("quality gate {label} must be an object");
    }
    Ok(())
}

fn require_str_eq(value: &Value, key: &str, expected: &str) -> Result<()> {
    let actual = value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("quality gate missing string field {key}"))?;
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("quality gate {key} must be {expected}, got {actual}");
    }
    Ok(())
}

fn require_substantive_str(value: &Value, key: &str) -> Result<()> {
    let actual = value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("quality gate missing string field {key}"))?;
    let trimmed = actual.trim();
    if trimmed.is_empty() {
        bail!("quality gate {key} must not be empty");
    }
    let normalized = trimmed.to_ascii_lowercase();
    if trimmed.len() < 16
        || matches!(
            normalized.as_str(),
            "todo" | "tbd" | "n/a" | "na" | "none" | "later" | "done" | "passed"
        )
    {
        bail!("quality gate {key} must contain substantive evidence");
    }
    Ok(())
}

fn require_empty_array(value: &Value, key: &str) -> Result<()> {
    match value.get(key).and_then(Value::as_array) {
        Some(items) if items.is_empty() => Ok(()),
        Some(_) => bail!("quality gate {key} must be empty"),
        None => bail!("quality gate missing array field {key}"),
    }
}

fn require_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    let items = value
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("quality gate missing array field {key}"))?;
    if items.is_empty() {
        bail!("quality gate {key} must not be empty");
    }
    items
        .iter()
        .map(|item| {
            let value = item
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .with_context(|| format!("quality gate {key} entries must be non-empty strings"))?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_artifact_refs(value: &Value, key: &str, artifact_root: &Path) -> Result<()> {
    for raw in require_string_array(value, key)? {
        let path = if Path::new(&raw).is_absolute() {
            PathBuf::from(&raw)
        } else {
            artifact_root.join(&raw)
        };
        let metadata = fs::metadata(&path).with_context(|| {
            format!("quality gate {key} artifact is missing: {}", path.display())
        })?;
        if !metadata.is_file() {
            bail!(
                "quality gate {key} artifact must be a file: {}",
                path.display()
            );
        }
        if metadata.len() == 0 {
            bail!(
                "quality gate {key} artifact must not be empty: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn completion_receipt(
    plan: &UltragoalPlan,
    goal_id: &str,
    evidence: &str,
    quality_gate: &Value,
    timestamp: &str,
) -> Result<CompletionReceipt> {
    let quality_gate_raw = serde_json::to_string(quality_gate)?;
    let quality_gate_sha256 = sha256_hex(quality_gate_raw.as_bytes());
    let evidence_sha256 = sha256_hex(evidence.as_bytes());
    let receipt_seed = format!("{goal_id}\n{timestamp}\n{quality_gate_sha256}\n{evidence_sha256}");
    let receipt_hash = sha256_hex(receipt_seed.as_bytes());
    Ok(CompletionReceipt {
        schema_version: 1,
        receipt_id: format!("ug-{}", &receipt_hash[..16]),
        goal_id: goal_id.to_string(),
        verified_at: timestamp.to_string(),
        brief_sha256: Some(plan.brief_sha256.clone()),
        source_plan_sha256: plan
            .source
            .as_ref()
            .and_then(|source| source.ralplan_plan_sha256.clone()),
        quality_gate_sha256,
        evidence_sha256,
    })
}

fn safe_part(value: impl AsRef<str>) -> String {
    let normalized = value
        .as_ref()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.trim().is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn scope_label(scope: ScopeArg) -> &'static str {
    match scope {
        ScopeArg::Global => "global",
        ScopeArg::Project => "project",
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn write_text_atomic(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt"),
        process::id()
    ));
    fs::write(&tmp, value)?;
    replace_file(&tmp, path)?;
    Ok(())
}

fn replace_file(tmp: &Path, path: &Path) -> Result<()> {
    match fs::rename(tmp, path) {
        Ok(()) => Ok(()),
        Err(_error) if path.exists() && tmp.exists() => {
            fs::remove_file(path)?;
            fs::rename(tmp, path).map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_goal_without_markers() {
        let goals = parse_goals("Ship a stable game\n\n- add tests").unwrap();

        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].title, "Ship a stable game");
        assert!(goals[0].objective.contains("add tests"));
    }

    #[test]
    fn parses_column_zero_goal_markers_only() {
        let goals = parse_goals(
            "preamble\n@goal: Board shell\nBuild the board.\n  @goal: ignored\n@goal Score model\nTrack score.",
        )
        .unwrap();

        assert_eq!(goals.len(), 2);
        assert_eq!(goals[0].title, "Board shell");
        assert!(goals[0].objective.contains("@goal: ignored"));
        assert_eq!(goals[1].title, "Score model");
    }

    #[test]
    fn quality_gate_requires_clear_architect_review() {
        let gate = json!({
            "architectReview": {
                "recommendation": "ITERATE",
                "architectureStatus": "CLEAR",
                "productStatus": "CLEAR",
                "codeStatus": "CLEAR",
                "evidence": "reviewed",
                "blockers": []
            },
            "executorQa": {
                "status": "passed",
                "e2eStatus": "passed",
                "redTeamStatus": "passed",
                "evidence": "tested",
                "blockers": []
            },
            "iteration": {
                "status": "passed",
                "fullRerun": true,
                "evidence": "rerun",
                "blockers": []
            }
        });

        assert!(validate_quality_gate(&gate, Path::new(".")).is_err());
    }
}
