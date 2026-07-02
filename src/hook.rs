use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::cli::{HookArgs, ScopeArg};

const DEEP_INTERVIEW: &str = "deep-interview";
const RALPLAN: &str = "ralplan";
const ULTRAGOAL: &str = "ultragoal";
const WORKFLOWS: &[&str] = &[DEEP_INTERVIEW, RALPLAN, ULTRAGOAL];
const MUTATION_GUARD_WORKFLOWS: &[&str] = &[DEEP_INTERVIEW, RALPLAN, ULTRAGOAL];

#[derive(Debug)]
pub struct HookOptions {
    pub runtime: String,
    pub event: String,
    pub matcher: String,
}

struct WorkflowPaths {
    session_id: String,
    workflow_dir: PathBuf,
    session_file: PathBuf,
    events_file: PathBuf,
}

pub fn run(args: HookArgs) -> Result<i32> {
    let _managed_marker = args.managed_marker;
    let state_dir = scoped_state_dir(args.scope, args.project_root.as_deref())?;
    let options = HookOptions {
        runtime: args.runtime,
        event: args.event,
        matcher: args.matcher.unwrap_or_default(),
    };

    if fs::create_dir_all(&state_dir).is_err() {
        return Ok(0);
    }

    let mut payload_text = String::new();
    io::stdin().read_to_string(&mut payload_text)?;

    let timestamp = timestamp();
    let payload = serde_json::from_str::<Value>(&payload_text).unwrap_or_else(|_| json!({}));
    let payload_bytes = payload_text.len();

    let safe_runtime = safe_part(&options.runtime);
    let safe_event = safe_part(&options.event);
    let payload_dir = state_dir
        .join("payloads")
        .join(&safe_runtime)
        .join(&safe_event);
    fs::create_dir_all(&payload_dir)?;
    let payload_file = unique_payload_path(&payload_dir);
    fs::write(&payload_file, &payload_text)?;

    let last_payload_file = state_dir.join(format!("last-{safe_runtime}-{safe_event}.json"));
    fs::write(&last_payload_file, &payload_text)?;

    append_jsonl(
        &state_dir.join("events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "runtime": options.runtime,
            "event": options.event,
            "matcher": options.matcher,
            "payload": payload_file,
            "last_payload": last_payload_file,
            "payload_bytes": payload_bytes,
        }),
    )?;

    record_conversation_event(
        &state_dir,
        &timestamp,
        &options,
        &payload,
        &payload_file,
        payload_bytes,
    )?;
    run_workflow_event(&state_dir, &timestamp, &options, &payload, &payload_file)
}

fn scoped_state_dir(scope: ScopeArg, project_root: Option<&Path>) -> Result<PathBuf> {
    match scope {
        ScopeArg::Project => {
            let Some(project_root) = project_root else {
                bail!("project scope hook requires --project-root");
            };
            let project_root = fs::canonicalize(project_root)?;
            let cwd = fs::canonicalize(env::current_dir()?)?;
            if !cwd.starts_with(&project_root) {
                bail!(
                    "project scope hook cwd {} is outside project root {}",
                    cwd.display(),
                    project_root.display()
                );
            }
            Ok(project_root.join(".agents").join("state").join("hooks"))
        }
        ScopeArg::Global => Ok(home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".megara")
            .join("state")
            .join("hooks")),
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| env::var_os("USERPROFILE").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unique_payload_path(payload_dir: &Path) -> PathBuf {
    let base = format!("{}-{}", timestamp_millis(), process::id());
    let mut path = payload_dir.join(format!("{base}.json"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = payload_dir.join(format!("{base}-{suffix}.json"));
    }
    path
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

pub(crate) fn append_jsonl(path: &Path, entry: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_vec(entry)?;
    line.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    Ok(())
}

fn record_conversation_event(
    state_dir: &Path,
    timestamp: &str,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
    payload_bytes: usize,
) -> Result<()> {
    let Some(role) = conversation_role(&options.event) else {
        return Ok(());
    };

    append_jsonl(
        &state_dir.join("conversation-events.jsonl"),
        &json!({
            "timestamp": timestamp,
            "runtime": options.runtime,
            "event": options.event,
            "role": role,
            "payload": payload_file,
            "payload_bytes": payload_bytes,
        }),
    )?;

    let field = if role == "user" {
        "prompt"
    } else {
        "last_assistant_message"
    };
    let Some(content) = payload.get(field).and_then(Value::as_str) else {
        return Ok(());
    };
    if content.trim().is_empty() {
        return Ok(());
    }

    let mut entry = Map::new();
    entry.insert("timestamp".to_string(), json!(timestamp));
    entry.insert("runtime".to_string(), json!(options.runtime));
    entry.insert("event".to_string(), json!(options.event));
    entry.insert("role".to_string(), json!(role));
    entry.insert("content".to_string(), json!(content));
    entry.insert("payload".to_string(), json!(payload_file));

    for key in ["session_id", "turn_id", "transcript_path", "cwd", "model"] {
        if let Some(value) = payload.get(key) {
            entry.insert(key.to_string(), value.clone());
        }
    }

    append_jsonl(&state_dir.join("conversation.jsonl"), &Value::Object(entry))
}

fn conversation_role(event: &str) -> Option<&'static str> {
    match event {
        "UserPromptSubmit" => Some("user"),
        "Stop" => Some("assistant"),
        _ => None,
    }
}

fn run_workflow_event(
    state_dir: &Path,
    timestamp: &str,
    options: &HookOptions,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    match options.event.as_str() {
        "Stop" => handle_stop(timestamp, state_dir, payload, payload_file),
        "UserPromptSubmit" => handle_user_prompt(timestamp, state_dir, payload, payload_file),
        "PreToolUse" => handle_pre_tool_use(timestamp, state_dir, payload, payload_file),
        _ => Ok(0),
    }
}

fn workflow_paths(state_dir: &Path, payload: &Value, skill: &str) -> WorkflowPaths {
    let session_id = payload
        .get("session_id")
        .or_else(|| payload.get("thread_id"))
        .or_else(|| payload.get("turn_id"))
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    let workflow_dir = workflow_base_dir(state_dir).join(skill);
    let session_file = workflow_dir.join(format!("{}.json", safe_part(&session_id)));
    let events_file = workflow_dir.join("events.jsonl");
    WorkflowPaths {
        session_id,
        workflow_dir,
        session_file,
        events_file,
    }
}

fn workflow_base_dir(state_dir: &Path) -> PathBuf {
    if state_dir.file_name().is_some_and(|name| name == "hooks") {
        state_dir.parent().unwrap_or(state_dir).join("workflows")
    } else {
        state_dir.join("workflows")
    }
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn handle_stop(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    for review in review_passes_from_text(text) {
        let paths = workflow_paths(state_dir, payload, RALPLAN);
        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(RALPLAN, timestamp, &paths.session_id, payload));
        persist_ralplan_review(timestamp, payload_file, &paths, review, &mut state)?;
        write_json_atomic(&paths.session_file, &state)?;
    }

    let terminal = workflow_state_from_text(text);

    if let Some(terminal) = terminal {
        let paths = workflow_paths(state_dir, payload, &terminal.skill);
        let mut state = load_json(&paths.session_file)
            .unwrap_or_else(|| new_state(&terminal.skill, timestamp, &paths.session_id, payload));
        match terminal.skill.as_str() {
            DEEP_INTERVIEW => handle_deep_interview_terminal(
                timestamp,
                text,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )?,
            RALPLAN => handle_ralplan_terminal(
                timestamp,
                text,
                payload_file,
                &paths,
                &terminal,
                &mut state,
            )?,
            ULTRAGOAL => {
                handle_generic_terminal(timestamp, payload_file, &paths, &terminal, &mut state)?
            }
            _ => return Ok(0),
        }
        write_json_atomic(&paths.session_file, &state)?;
        return Ok(0);
    }

    let Some(question) = question_from_text(timestamp, text, payload_file) else {
        return Ok(0);
    };

    let paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    let mut state = load_json(&paths.session_file)
        .unwrap_or_else(|| new_state(DEEP_INTERVIEW, timestamp, &paths.session_id, payload));
    {
        let question_id = question
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let round = question.get("round").cloned().unwrap_or(Value::Null);
        let component = question.get("component").cloned().unwrap_or(Value::Null);
        let dimension = question.get("dimension").cloned().unwrap_or(Value::Null);
        upsert_question(timestamp, &mut state, question);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "question_pending",
                "session_id": paths.session_id,
                "question_id": question_id,
                "round": round,
                "component": component,
                "dimension": dimension,
                "payload": payload_file,
            }),
        )?;
    }

    write_json_atomic(&paths.session_file, &state)?;
    Ok(0)
}

fn handle_user_prompt(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    let Some(prompt) = payload.get("prompt").and_then(Value::as_str) else {
        return Ok(0);
    };
    if prompt.trim().is_empty() {
        return Ok(0);
    }

    let ralplan_paths = workflow_paths(state_dir, payload, RALPLAN);
    if let Some(mut state) = load_json(&ralplan_paths.session_file) {
        if let Some(decision) =
            apply_ralplan_prompt_decision(timestamp, &mut state, prompt, payload_file)
        {
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| "unknown-session".to_string());
            write_json_atomic(&ralplan_paths.session_file, &state)?;
            append_jsonl(
                &ralplan_paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": decision.event,
                    "session_id": session_id,
                    "handoff_target": decision.handoff_target,
                    "plan_id": state.get("plan_id").cloned().unwrap_or(Value::Null),
                    "plan_sha256": state.get("plan_sha256").cloned().unwrap_or(Value::Null),
                    "payload": payload_file,
                }),
            )?;
            return Ok(0);
        }
    }

    if is_deep_interview_approval_for_ralplan(prompt) {
        let mut state = load_json(&ralplan_paths.session_file)
            .unwrap_or_else(|| new_state(RALPLAN, timestamp, &ralplan_paths.session_id, payload));
        require_ralplan_input_lock(timestamp, &mut state, payload_file);
        let session_id = state
            .get("session_id")
            .map(value_to_string)
            .unwrap_or_else(|| "unknown-session".to_string());
        write_json_atomic(&ralplan_paths.session_file, &state)?;
        append_jsonl(
            &ralplan_paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "input_lock_required",
                "session_id": session_id,
                "required_workflow": DEEP_INTERVIEW,
                "payload": payload_file,
            }),
        )?;
        return Ok(0);
    }

    let deep_paths = workflow_paths(state_dir, payload, DEEP_INTERVIEW);
    if let Some(mut state) = load_json(&deep_paths.session_file) {
        if let Some(question_id) =
            answer_pending_question(timestamp, &mut state, prompt, payload_file)
        {
            let session_id = state
                .get("session_id")
                .map(value_to_string)
                .unwrap_or_else(|| "unknown-session".to_string());
            write_json_atomic(&deep_paths.session_file, &state)?;
            append_jsonl(
                &deep_paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "question_answered",
                    "session_id": session_id,
                    "question_id": question_id,
                    "payload": payload_file,
                }),
            )?;
            return Ok(0);
        }
    }
    Ok(0)
}

fn handle_pre_tool_use(
    timestamp: &str,
    state_dir: &Path,
    payload: &Value,
    payload_file: &Path,
) -> Result<i32> {
    if env::var("MEGARA_MUTATION_GUARD").unwrap_or_else(|_| "block".to_string()) == "off" {
        return Ok(0);
    }
    let Some(mutation) = mutation_signal(payload) else {
        return Ok(0);
    };
    let Some((skill, state, events_file)) = active_workflow_state(state_dir, payload) else {
        return Ok(0);
    };

    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    append_jsonl(
        &events_file,
        &json!({
            "timestamp": timestamp,
            "event": "mutation_blocked",
            "session_id": session_id,
            "skill": skill,
            "phase": state.get("phase").cloned().unwrap_or(Value::Null),
            "mutation_kind": mutation.kind,
            "mutation_value": mutation.value,
            "payload": payload_file,
        }),
    )?;

    let guidance = if skill == ULTRAGOAL {
        "run `megara ultragoal complete-goals` and enter an active goal before mutating files"
    } else {
        "approve, refine, complete, or cancel the workflow before mutating files"
    };
    eprintln!("MEGARA mutation guard: {skill} is active. {guidance}.");
    if env::var("MEGARA_MUTATION_GUARD").unwrap_or_else(|_| "block".to_string()) == "warn" {
        Ok(0)
    } else {
        Ok(42)
    }
}

fn handle_deep_interview_terminal(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    let spec = persist_crystallized_spec(
        timestamp,
        &paths.workflow_dir,
        &paths.session_id,
        terminal,
        text,
        payload_file,
    )?;
    if terminal.status == "crystallized" && spec.is_none() {
        reject_crystallized_without_spec(timestamp, state);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "spec_missing",
                "session_id": paths.session_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }

    update_terminal_state(timestamp, state, terminal, spec.as_ref());
    let mut entry = json!({
        "timestamp": timestamp,
        "event": "workflow_state",
        "session_id": paths.session_id,
        "skill": terminal.skill,
        "status": terminal.status,
        "payload": payload_file,
    });
    if let Some(spec) = spec {
        entry["spec_path"] = json!(spec.path);
        entry["spec_sha256"] = json!(spec.sha256);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "spec_persisted",
                "session_id": paths.session_id,
                "path": spec.path,
                "sha256": spec.sha256,
                "payload": payload_file,
            }),
        )?;
    }
    append_jsonl(&paths.events_file, &entry)
}

fn handle_ralplan_terminal(
    timestamp: &str,
    text: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    let plan_gate = plan_gate_from_text(text);
    let plan_id = terminal
        .plan_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| plan_gate.as_ref().map(|gate| gate.id.as_str()))
        .unwrap_or("rp-plan")
        .to_string();
    if terminal.status == "pending_approval" {
        if let Some(blocker) = active_deep_interview_state(paths) {
            reject_ralplan_handoff_not_ready(timestamp, state, &plan_id, &blocker);
            append_jsonl(
                &paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "handoff_blocked",
                    "session_id": paths.session_id,
                    "plan_id": plan_id,
                    "blocked_by": DEEP_INTERVIEW,
                    "blocked_phase": blocker.get("phase").cloned().unwrap_or(Value::Null),
                    "blocked_status": blocker.get("status").cloned().unwrap_or(Value::Null),
                    "payload": payload_file,
                }),
            )?;
            return Ok(());
        }
    }
    let linked_spec = linked_deep_interview_spec(paths);
    if terminal.status == "pending_approval" {
        if let Some(reason) = ralplan_input_lock_blocker(state, linked_spec.as_ref(), text) {
            reject_ralplan_input_lock(timestamp, state, &plan_id, reason);
            append_jsonl(
                &paths.events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "input_lock_blocked",
                    "session_id": paths.session_id,
                    "plan_id": plan_id,
                    "reason": reason,
                    "payload": payload_file,
                }),
            )?;
            return Ok(());
        }
    }
    if terminal.status == "pending_approval" && !ralplan_reviews_ready(state) {
        reject_ralplan_without_reviews(timestamp, state, &plan_id);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "review_incomplete",
                "session_id": paths.session_id,
                "plan_id": plan_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }
    let plan = persist_pending_plan(
        timestamp,
        paths,
        &plan_id,
        terminal,
        text,
        payload_file,
        linked_spec.as_ref(),
    )?;

    if terminal.status == "pending_approval" && plan.is_none() {
        reject_ralplan_without_plan(timestamp, state, &plan_id);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "plan_missing",
                "session_id": paths.session_id,
                "plan_id": plan_id,
                "status": terminal.status,
                "payload": payload_file,
            }),
        )?;
        return Ok(());
    }

    update_terminal_state(timestamp, state, terminal, None);
    state["plan_id"] = json!(plan_id);
    if terminal.status == "pending_approval" {
        state["active"] = json!(true);
        state["phase"] = json!("pending_approval");
        state["approval_status"] = json!("pending");
    }
    if let Some(gate) = plan_gate {
        state["plan_gate"] = json!({
            "id": gate.id,
            "status": gate.status,
            "question": gate.question,
            "options": gate.options,
            "free_text": gate.free_text,
        });
    }
    if let Some(spec) = &linked_spec {
        state["input_spec_path"] = json!(spec.path);
        state["input_spec_sha256"] = json!(spec.sha256);
        state["input_spec_persisted_at"] = json!(spec.persisted_at);
    }

    let mut entry = json!({
        "timestamp": timestamp,
        "event": "workflow_state",
        "session_id": paths.session_id,
        "skill": terminal.skill,
        "status": terminal.status,
        "plan_id": plan_id,
        "payload": payload_file,
    });
    if let Some(spec) = &linked_spec {
        entry["input_spec_path"] = json!(spec.path);
        entry["input_spec_sha256"] = json!(spec.sha256);
    }
    if let Some(plan) = plan {
        state["plan_path"] = json!(plan.path);
        state["plan_sha256"] = json!(plan.sha256);
        state["plan_persisted_at"] = json!(plan.persisted_at);
        state["plan_payload"] = json!(plan.payload);
        entry["plan_path"] = json!(plan.path);
        entry["plan_sha256"] = json!(plan.sha256);
        append_jsonl(
            &paths.events_file,
            &json!({
                "timestamp": timestamp,
                "event": "plan_persisted",
                "session_id": paths.session_id,
                "plan_id": plan_id,
                "path": plan.path,
                "sha256": plan.sha256,
                "input_spec_path": linked_spec.as_ref().map(|spec| spec.path.as_str()),
                "input_spec_sha256": linked_spec.as_ref().map(|spec| spec.sha256.as_str()),
                "payload": payload_file,
            }),
        )?;
    }
    append_jsonl(&paths.events_file, &entry)
}

fn handle_generic_terminal(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    terminal: &TerminalState,
    state: &mut Value,
) -> Result<()> {
    update_terminal_state(timestamp, state, terminal, None);
    append_jsonl(
        &paths.events_file,
        &json!({
            "timestamp": timestamp,
            "event": "workflow_state",
            "session_id": paths.session_id,
            "skill": terminal.skill,
            "status": terminal.status,
            "payload": payload_file,
        }),
    )
}

fn persist_ralplan_review(
    timestamp: &str,
    payload_file: &Path,
    paths: &WorkflowPaths,
    review: ReviewPass,
    state: &mut Value,
) -> Result<()> {
    let review_path = unique_review_path(
        &paths.workflow_dir,
        &paths.session_id,
        &review.role,
        review.round,
        timestamp,
    );
    let mut content = [
        "---".to_string(),
        "skill: \"ralplan\"".to_string(),
        format!("session_id: {}", yaml_string(&paths.session_id)),
        format!("role: {}", yaml_string(&review.role)),
        format!("round: {}", review.round),
        format!("verdict: {}", yaml_string(&review.verdict)),
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        format!("# {} review", review.role),
        String::new(),
        format!("Verdict: `{}`", review.verdict),
        String::new(),
        "## Summary".to_string(),
        String::new(),
        review.summary.clone(),
        String::new(),
        "## Required Fixes".to_string(),
        String::new(),
        review
            .required_fixes
            .iter()
            .map(|fix| format!("- {fix}"))
            .collect::<Vec<_>>()
            .join("\n"),
    ]
    .join("\n");
    content.push('\n');

    write_text_atomic(&review_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    let review_entry = json!({
        "timestamp": timestamp,
        "event": "review_persisted",
        "session_id": paths.session_id,
        "skill": RALPLAN,
        "role": review.role,
        "round": review.round,
        "verdict": review.verdict,
        "summary": review.summary,
        "required_fixes": review.required_fixes,
        "path": review_path,
        "sha256": sha256,
        "payload": payload_file,
    });
    append_jsonl(
        &paths.workflow_dir.join("reviews").join("index.jsonl"),
        &review_entry,
    )?;
    append_jsonl(&paths.events_file, &review_entry)?;

    let mut reviews = state
        .get("reviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    reviews.push(json!({
        "role": review_entry["role"].clone(),
        "round": review_entry["round"].clone(),
        "verdict": review_entry["verdict"].clone(),
        "summary": review_entry["summary"].clone(),
        "required_fixes": review_entry["required_fixes"].clone(),
        "path": review_entry["path"].clone(),
        "sha256": review_entry["sha256"].clone(),
        "persisted_at": timestamp,
        "payload": payload_file,
    }));
    state["reviews"] = json!(reviews);
    state["active"] = json!(true);
    state["phase"] = json!("reviewing");
    state["status"] = json!("reviewing");
    state["updated_at"] = json!(timestamp);
    Ok(())
}

fn linked_deep_interview_spec(paths: &WorkflowPaths) -> Option<LinkedSpec> {
    let state = deep_interview_state(paths)?;
    if state.get("status").and_then(Value::as_str) != Some("crystallized") {
        return None;
    }
    Some(LinkedSpec {
        path: state.get("spec_path")?.as_str()?.to_string(),
        sha256: state.get("spec_sha256")?.as_str()?.to_string(),
        persisted_at: state
            .get("spec_persisted_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn ralplan_input_lock_blocker(
    state: &Value,
    linked_spec: Option<&LinkedSpec>,
    text: &str,
) -> Option<&'static str> {
    if state.get("requires_input_lock").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let Some(spec) = linked_spec else {
        return Some("missing_persisted_deep_interview_lock");
    };
    let Some(input_sha256) = workflow_state_field(text, "input_spec_sha256") else {
        return Some("missing_input_spec_sha256");
    };
    if input_sha256 != spec.sha256 {
        return Some("input_spec_sha256_mismatch");
    }
    None
}

fn active_deep_interview_state(paths: &WorkflowPaths) -> Option<Value> {
    let state = deep_interview_state(paths)?;
    if state.get("active").and_then(Value::as_bool) == Some(true)
        && state.get("status").and_then(Value::as_str) != Some("crystallized")
    {
        Some(state)
    } else {
        None
    }
}

fn deep_interview_state(paths: &WorkflowPaths) -> Option<Value> {
    let workflow_base = paths.workflow_dir.parent()?;
    let deep_state_path = workflow_base
        .join(DEEP_INTERVIEW)
        .join(format!("{}.json", safe_part(&paths.session_id)));
    let state = load_json(&deep_state_path)?;
    if state.get("skill").and_then(Value::as_str) != Some(DEEP_INTERVIEW) {
        return None;
    }
    Some(state)
}

fn is_deep_interview_approval_for_ralplan(prompt: &str) -> bool {
    parse_blocks(prompt, "Megara Approval Gate:")
        .into_iter()
        .any(|block| {
            field_eq(&block, "approved_workflow", DEEP_INTERVIEW)
                && field_eq(&block, "next_workflow", RALPLAN)
                && field_eq(&block, "approved_status", "crystallized")
        })
}

fn workflow_state_field(text: &str, key: &str) -> Option<String> {
    let block = parse_block(text, "Megara Workflow State:")?;
    let value = block.fields.get(key)?.trim().trim_matches('"').to_string();
    (!value.is_empty()).then_some(value)
}

fn field_eq(block: &Block, key: &str, expected: &str) -> bool {
    block
        .fields
        .get(key)
        .map(|value| {
            value
                .trim()
                .trim_matches('"')
                .eq_ignore_ascii_case(expected)
        })
        .unwrap_or(false)
}

fn ralplan_reviews_ready(state: &Value) -> bool {
    let Some(reviews) = state.get("reviews").and_then(Value::as_array) else {
        return false;
    };
    let mut latest = BTreeMap::<&str, &str>::new();
    for review in reviews {
        let Some(role) = review.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(verdict) = review.get("verdict").and_then(Value::as_str) else {
            continue;
        };
        latest.insert(role, verdict);
    }

    let planner_ready = latest
        .get("planner")
        .is_some_and(|verdict| matches!(*verdict, "DRAFT" | "CLEAR" | "WATCH" | "OKAY"));
    let architect_ready = latest
        .get("architect")
        .is_some_and(|verdict| matches!(*verdict, "CLEAR" | "WATCH" | "OKAY"));
    let critic_ready = latest
        .get("critic")
        .is_some_and(|verdict| matches!(*verdict, "OKAY"));

    planner_ready && architect_ready && critic_ready
}

fn active_workflow_state(
    state_dir: &Path,
    payload: &Value,
) -> Option<(&'static str, Value, PathBuf)> {
    for &skill in MUTATION_GUARD_WORKFLOWS {
        let paths = workflow_paths(state_dir, payload, skill);
        if let Some(state) = load_json(&paths.session_file) {
            if mutation_guard_applies(skill, &state) {
                return Some((skill, state, paths.events_file));
            }
        }
    }
    None
}

fn mutation_guard_applies(skill: &'static str, state: &Value) -> bool {
    if state.get("active").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    if skill != ULTRAGOAL {
        return true;
    }
    matches!(
        state.get("phase").and_then(Value::as_str),
        Some("goal_planning" | "planning" | "initialized" | "handoff")
    )
}

struct RalplanPromptDecision {
    event: &'static str,
    handoff_target: Value,
}

pub(crate) struct ApprovalGate {
    pub(crate) plan_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) handoff_target: String,
}

#[derive(Debug)]
struct LinkedSpec {
    path: String,
    sha256: String,
    persisted_at: String,
}

fn apply_ralplan_prompt_decision(
    timestamp: &str,
    state: &mut Value,
    prompt: &str,
    payload_file: &Path,
) -> Option<RalplanPromptDecision> {
    if state.get("skill").and_then(Value::as_str) != Some(RALPLAN) {
        return None;
    }
    if state.get("active").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    if state.get("phase").and_then(Value::as_str) != Some("pending_approval") {
        return None;
    }

    if let Some(gate) = approval_gate_from_text(prompt) {
        let current_plan_id = state
            .get("plan_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let current_plan_sha256 = state
            .get("plan_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if gate.plan_id == current_plan_id
            && gate.plan_sha256 == current_plan_sha256
            && matches!(gate.handoff_target.as_str(), "ultragoal" | "team")
        {
            let plan_sha256 = json!(gate.plan_sha256);
            approve_ralplan(
                timestamp,
                state,
                &gate.handoff_target,
                plan_sha256,
                payload_file,
            );
            return Some(RalplanPromptDecision {
                event: "plan_approved",
                handoff_target: json!(gate.handoff_target),
            });
        }

        state["approval_status"] = json!("approval_gate_mismatch");
        state["phase"] = json!("pending_approval");
        state["updated_at"] = json!(timestamp);
        state["last_approval_payload"] = json!(payload_file);
        return Some(RalplanPromptDecision {
            event: "plan_approval_rejected",
            handoff_target: Value::Null,
        });
    }

    let normalized = prompt.to_ascii_lowercase();
    let plan_sha256 = state.get("plan_sha256").cloned().unwrap_or(Value::Null);
    if normalized.contains("approve_ultragoal")
        || (normalized.contains("approve") && normalized.contains("ultragoal"))
        || normalized.contains("ultragoal 승인")
    {
        approve_ralplan(timestamp, state, "ultragoal", plan_sha256, payload_file);
        return Some(RalplanPromptDecision {
            event: "plan_approved",
            handoff_target: json!("ultragoal"),
        });
    }
    if normalized.contains("approve_team")
        || (normalized.contains("approve") && normalized.contains("team"))
        || normalized.contains("team 승인")
    {
        approve_ralplan(timestamp, state, "team", plan_sha256, payload_file);
        return Some(RalplanPromptDecision {
            event: "plan_approved",
            handoff_target: json!("team"),
        });
    }
    if normalized.contains("refine")
        || normalized.contains("iterate")
        || normalized.contains("보완")
        || normalized.contains("수정")
    {
        state["reviews"] = json!([]);
        remove_state_fields(
            state,
            &[
                "plan_gate",
                "plan_path",
                "plan_sha256",
                "plan_persisted_at",
                "plan_payload",
                "input_spec_path",
                "input_spec_sha256",
                "input_spec_persisted_at",
            ],
        );
        state["approval_status"] = json!("refine_requested");
        state["phase"] = json!("refining");
        state["updated_at"] = json!(timestamp);
        state["last_approval_payload"] = json!(payload_file);
        return Some(RalplanPromptDecision {
            event: "plan_refine_requested",
            handoff_target: Value::Null,
        });
    }
    if normalized.contains("stop_pending")
        || normalized.contains("pending")
        || normalized.contains("보류")
    {
        state["approval_status"] = json!("pending");
        state["phase"] = json!("pending_approval");
        state["updated_at"] = json!(timestamp);
        state["last_approval_payload"] = json!(payload_file);
        return Some(RalplanPromptDecision {
            event: "plan_left_pending",
            handoff_target: Value::Null,
        });
    }

    None
}

fn approve_ralplan(
    timestamp: &str,
    state: &mut Value,
    handoff_target: &str,
    plan_sha256: Value,
    payload_file: &Path,
) {
    state["active"] = json!(false);
    state["phase"] = json!("approved");
    state["status"] = json!("approved");
    state["approval_status"] = json!("approved");
    state["approved_handoff_target"] = json!(handoff_target);
    if let Some(plan_id) = state.get("plan_id").cloned() {
        state["approved_plan_id"] = plan_id;
    }
    state["approved_plan_sha256"] = plan_sha256;
    state["approved_at"] = json!(timestamp);
    state["closed_at"] = json!(timestamp);
    state["last_approval_payload"] = json!(payload_file);
    state["updated_at"] = json!(timestamp);
}

fn remove_state_fields(state: &mut Value, fields: &[&str]) {
    let Some(object) = state.as_object_mut() else {
        return;
    };
    for field in fields {
        object.remove(*field);
    }
}

#[derive(Debug)]
pub(crate) struct Block {
    pub(crate) fields: BTreeMap<String, String>,
    lists: BTreeMap<String, Vec<String>>,
}

fn parse_block(text: &str, marker: &str) -> Option<Block> {
    parse_blocks(text, marker).into_iter().next()
}

pub(crate) fn parse_blocks(text: &str, marker: &str) -> Vec<Block> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if lines[index].trim() != marker {
            index += 1;
            continue;
        }

        index += 1;
        let mut fields = BTreeMap::new();
        let mut lists = BTreeMap::<String, Vec<String>>::new();
        let mut current_key = String::new();
        let mut saw_field = false;

        while index < lines.len() {
            let raw = lines[index];
            if raw.trim().is_empty() {
                index += 1;
                if saw_field {
                    break;
                }
                continue;
            }

            if (raw.starts_with("  - ") || raw.starts_with("    - ")) && !current_key.is_empty() {
                if let Some((_, value)) = raw.split_once("- ") {
                    lists
                        .entry(current_key.clone())
                        .or_default()
                        .push(clean_block_value(value));
                }
                index += 1;
                continue;
            }

            let stripped = raw.trim();
            if !stripped.starts_with("- ") {
                break;
            }

            let Some((key, value)) = stripped[2..].split_once(':') else {
                index += 1;
                continue;
            };
            let key = key.trim().to_ascii_lowercase().replace('-', "_");
            current_key = key.clone();
            saw_field = true;
            if value.trim().is_empty() {
                lists.entry(key).or_default();
            } else {
                fields.insert(key, clean_block_value(value));
            }
            index += 1;
        }

        if saw_field {
            blocks.push(Block { fields, lists });
        }
    }

    blocks
}

fn clean_block_value(value: &str) -> String {
    let mut value = value.trim();
    while let Some(stripped) = value
        .strip_suffix("</input>")
        .or_else(|| value.strip_suffix("</codex_delegation>"))
    {
        value = stripped.trim_end();
    }
    value.to_string()
}

fn block_list(block: &Block, key: &str) -> Vec<String> {
    block
        .lists
        .get(key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect()
}

fn question_from_text(timestamp: &str, text: &str, payload_file: &Path) -> Option<Value> {
    let block = parse_block(text, "Megara Question Gate:")?;
    let question_id = block.fields.get("id")?.trim();
    let question = block.fields.get("question")?.trim();
    if question_id.is_empty() || question.is_empty() {
        return None;
    }

    Some(json!({
        "id": question_id,
        "round": normalize_round(block.fields.get("round").map(String::as_str)),
        "component": block.fields.get("component").map(String::as_str).unwrap_or("").trim(),
        "dimension": block.fields.get("dimension").map(String::as_str).unwrap_or("").trim(),
        "question": question,
        "options": block_list(&block, "options"),
        "free_text": parse_bool(block.fields.get("free_text").map(String::as_str).unwrap_or("false")),
        "status": "pending",
        "asked_at": timestamp,
        "payload": payload_file,
    }))
}

fn normalize_round(value: Option<&str>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    value
        .trim()
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| json!(value))
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

#[derive(Debug)]
struct TerminalState {
    skill: String,
    status: String,
    ambiguity: String,
    next: String,
    plan_id: Option<String>,
}

struct PlanGate {
    id: String,
    status: String,
    question: String,
    options: Vec<String>,
    free_text: bool,
}

struct ReviewPass {
    role: String,
    round: i64,
    verdict: String,
    summary: String,
    required_fixes: Vec<String>,
}

fn review_passes_from_text(text: &str) -> Vec<ReviewPass> {
    parse_blocks(text, "Megara Review Pass:")
        .into_iter()
        .filter_map(review_pass_from_block)
        .collect()
}

fn review_pass_from_block(block: Block) -> Option<ReviewPass> {
    let role = normalize_review_role(block.fields.get("role")?.trim())?;
    let verdict = normalize_review_verdict(block.fields.get("verdict")?.trim())?;
    let summary = block.fields.get("summary")?.trim();
    if role.is_empty() || verdict.is_empty() || summary.is_empty() {
        return None;
    }
    let round = block
        .fields
        .get("round")
        .and_then(|round| round.trim().parse::<i64>().ok())
        .filter(|round| *round > 0)?;
    Some(ReviewPass {
        role,
        round,
        verdict,
        summary: summary.to_string(),
        required_fixes: {
            let fixes = block_list(&block, "required_fixes");
            if fixes.is_empty() {
                vec!["none".to_string()]
            } else {
                fixes
            }
        },
    })
}

fn normalize_review_role(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "planner" | "architect" | "critic").then_some(normalized)
}

fn normalize_review_verdict(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    matches!(
        normalized.as_str(),
        "DRAFT" | "CLEAR" | "WATCH" | "BLOCK" | "OKAY" | "ITERATE" | "REJECT"
    )
    .then_some(normalized)
}

fn plan_gate_from_text(text: &str) -> Option<PlanGate> {
    let block = parse_block(text, "Megara Plan Gate:")?;
    let id = block.fields.get("id")?.trim();
    if id.is_empty() {
        return None;
    }
    Some(PlanGate {
        id: id.to_string(),
        status: block
            .fields
            .get("status")
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| "pending_approval".to_string()),
        question: block
            .fields
            .get("question")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        options: block_list(&block, "options"),
        free_text: parse_bool(
            block
                .fields
                .get("free_text")
                .map(String::as_str)
                .unwrap_or("false"),
        ),
    })
}

pub(crate) fn approval_gate_from_text(text: &str) -> Option<ApprovalGate> {
    let block = parse_block(text, "Megara Approval Gate:")?;
    let plan_id = block.fields.get("plan_id")?.trim();
    let plan_sha256 = block.fields.get("plan_sha256")?.trim();
    let handoff_target = block.fields.get("handoff_target")?.trim();
    if plan_id.is_empty() || plan_sha256.len() != 64 || handoff_target.is_empty() {
        return None;
    }
    Some(ApprovalGate {
        plan_id: plan_id.to_string(),
        plan_sha256: plan_sha256.to_string(),
        handoff_target: handoff_target.to_ascii_lowercase(),
    })
}

fn workflow_state_from_text(text: &str) -> Option<TerminalState> {
    let block = parse_block(text, "Megara Workflow State:")?;
    let skill = block.fields.get("skill")?.trim();
    if !WORKFLOWS.contains(&skill) {
        return None;
    }
    let status = block.fields.get("status")?.trim().to_ascii_lowercase();
    if status.is_empty() {
        return None;
    }
    Some(TerminalState {
        skill: skill.to_string(),
        status,
        ambiguity: block
            .fields
            .get("ambiguity")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        next: block
            .fields
            .get("next")
            .map(|value| value.trim().to_string())
            .unwrap_or_default(),
        plan_id: block
            .fields
            .get("plan_id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

fn text_before_block(text: &str, marker: &str) -> String {
    for (index, line) in text.lines().enumerate() {
        if line.trim() == marker {
            return text
                .lines()
                .take(index)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
        }
    }
    text.trim().to_string()
}

#[derive(Debug)]
struct PersistedSpec {
    path: String,
    sha256: String,
    persisted_at: String,
    payload: String,
}

#[derive(Debug)]
struct PersistedPlan {
    path: String,
    sha256: String,
    persisted_at: String,
    payload: String,
}

fn persist_crystallized_spec(
    timestamp: &str,
    workflow_dir: &Path,
    session_id: &str,
    terminal: &TerminalState,
    text: &str,
    payload_file: &Path,
) -> Result<Option<PersistedSpec>> {
    if terminal.status != "crystallized" || text.trim().is_empty() {
        return Ok(None);
    }
    if text_before_block(text, "Megara Workflow State:").is_empty() {
        return Ok(None);
    }

    let mut content = [
        "---".to_string(),
        "skill: \"deep-interview\"".to_string(),
        format!("session_id: {}", yaml_string(session_id)),
        "status: \"crystallized\"".to_string(),
        format!("ambiguity: {}", yaml_string(&terminal.ambiguity)),
        format!("next: {}", yaml_string(&terminal.next)),
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        text.trim().to_string(),
    ]
    .join("\n");
    content.push('\n');

    let spec_path = unique_spec_path(workflow_dir, session_id, timestamp);
    write_text_atomic(&spec_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    append_jsonl(
        &workflow_dir.join("specs").join("index.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "spec_persisted",
            "session_id": session_id,
            "skill": DEEP_INTERVIEW,
            "status": "crystallized",
            "path": spec_path,
            "sha256": sha256,
            "payload": payload_file,
        }),
    )?;

    Ok(Some(PersistedSpec {
        path: spec_path.display().to_string(),
        sha256,
        persisted_at: timestamp.to_string(),
        payload: payload_file.display().to_string(),
    }))
}

fn persist_pending_plan(
    timestamp: &str,
    paths: &WorkflowPaths,
    plan_id: &str,
    terminal: &TerminalState,
    text: &str,
    payload_file: &Path,
    linked_spec: Option<&LinkedSpec>,
) -> Result<Option<PersistedPlan>> {
    if terminal.status != "pending_approval" || text.trim().is_empty() {
        return Ok(None);
    }
    let plan_body = text_before_first_workflow_block(text);
    if plan_body.is_empty() {
        return Ok(None);
    }

    let mut header = vec![
        "---".to_string(),
        "skill: \"ralplan\"".to_string(),
        format!("session_id: {}", yaml_string(&paths.session_id)),
        format!("plan_id: {}", yaml_string(plan_id)),
        "status: \"pending_approval\"".to_string(),
        format!("next: {}", yaml_string(&terminal.next)),
    ];
    if let Some(spec) = linked_spec {
        header.push(format!("input_spec_path: {}", yaml_string(&spec.path)));
        header.push(format!("input_spec_sha256: {}", yaml_string(&spec.sha256)));
    }
    header.extend([
        format!("persisted_at: {}", yaml_string(timestamp)),
        format!("payload: {}", yaml_string(payload_file.display())),
        "---".to_string(),
        String::new(),
        plan_body,
    ]);
    let mut content = header.join("\n");
    content.push('\n');

    let plan_path = unique_plan_path(&paths.workflow_dir, &paths.session_id, plan_id, timestamp);
    write_text_atomic(&plan_path, &content)?;
    let sha256 = sha256_hex(content.as_bytes());
    append_jsonl(
        &paths.workflow_dir.join("plans").join("index.jsonl"),
        &json!({
            "timestamp": timestamp,
            "event": "plan_persisted",
            "session_id": paths.session_id,
            "skill": RALPLAN,
            "status": "pending_approval",
            "plan_id": plan_id,
            "path": plan_path,
            "sha256": sha256,
            "input_spec_path": linked_spec.map(|spec| spec.path.as_str()),
            "input_spec_sha256": linked_spec.map(|spec| spec.sha256.as_str()),
            "payload": payload_file,
        }),
    )?;

    Ok(Some(PersistedPlan {
        path: plan_path.display().to_string(),
        sha256,
        persisted_at: timestamp.to_string(),
        payload: payload_file.display().to_string(),
    }))
}

fn yaml_string(value: impl std::fmt::Display) -> String {
    serde_json::to_string(&value.to_string()).unwrap_or_else(|_| "\"\"".to_string())
}

pub(crate) fn text_before_first_workflow_block(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() == "Megara Plan Gate:" && marker_has_immediate_fields(&lines, index) {
            let tail = lines[index..].join("\n");
            if plan_gate_from_text(&tail).is_some() && workflow_state_from_text(&tail).is_some() {
                end = index;
                break;
            }
        }
    }
    lines
        .into_iter()
        .take(end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn marker_has_immediate_fields(lines: &[&str], marker_index: usize) -> bool {
    let mut index = marker_index + 1;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }
    lines
        .get(index)
        .is_some_and(|line| line.trim_start().starts_with("- "))
}

fn unique_spec_path(workflow_dir: &Path, session_id: &str, timestamp: &str) -> PathBuf {
    let specs_dir = workflow_dir.join("specs");
    let base = format!(
        "deep-interview-{}-{}",
        safe_part(session_id),
        safe_part(timestamp)
    );
    let mut path = specs_dir.join(format!("{base}.md"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = specs_dir.join(format!("{base}-{suffix}.md"));
    }
    path
}

fn unique_plan_path(
    workflow_dir: &Path,
    session_id: &str,
    plan_id: &str,
    timestamp: &str,
) -> PathBuf {
    let plans_dir = workflow_dir.join("plans");
    let base = format!(
        "ralplan-{}-{}-{}",
        safe_part(session_id),
        safe_part(plan_id),
        safe_part(timestamp)
    );
    let mut path = plans_dir.join(format!("{base}.md"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = plans_dir.join(format!("{base}-{suffix}.md"));
    }
    path
}

fn unique_review_path(
    workflow_dir: &Path,
    session_id: &str,
    role: &str,
    round: i64,
    timestamp: &str,
) -> PathBuf {
    let reviews_dir = workflow_dir.join("reviews");
    let base = format!(
        "ralplan-review-{}-{}-r{}-{}",
        safe_part(session_id),
        safe_part(role),
        round,
        safe_part(timestamp)
    );
    let mut path = reviews_dir.join(format!("{base}.md"));
    let mut suffix = 0;
    while path.exists() {
        suffix += 1;
        path = reviews_dir.join(format!("{base}-{suffix}.md"));
    }
    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn load_json(path: &Path) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .filter(Value::is_object)
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("json"),
        process::id()
    ));
    let mut content = serde_json::to_string_pretty(value)?;
    content.push('\n');
    fs::write(&tmp, content)?;
    replace_file(&tmp, path)?;
    Ok(())
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

fn new_state(skill: &str, timestamp: &str, session_id: &str, payload: &Value) -> Value {
    json!({
        "version": 1,
        "skill": skill,
        "session_id": session_id,
        "cwd": payload.get("cwd").cloned().unwrap_or(Value::Null),
        "active": true,
        "phase": "initialized",
        "pending_question": Value::Null,
        "questions": [],
        "updated_at": timestamp,
    })
}

fn upsert_question(timestamp: &str, state: &mut Value, question: Value) {
    let pending_id = state.get("pending_question").and_then(|pending| {
        (pending.get("status").and_then(Value::as_str) == Some("pending"))
            .then(|| pending.get("id").map(value_to_string))
            .flatten()
    });

    if let Some(pending_id) = pending_id {
        if let Some(questions) = state.get_mut("questions").and_then(Value::as_array_mut) {
            if let Some(existing) = questions.iter_mut().find(|existing| {
                existing.get("id").map(value_to_string) == Some(pending_id.clone())
                    && existing.get("status").and_then(Value::as_str) == Some("pending")
            }) {
                existing["status"] = json!("superseded");
                existing["superseded_at"] = json!(timestamp);
            }
        }
    }

    let question_id = question.get("id").map(value_to_string).unwrap_or_default();
    let mut questions = state
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|existing| existing.get("id").map(value_to_string) != Some(question_id.clone()))
        .collect::<Vec<_>>();
    questions.push(question.clone());

    state["questions"] = json!(questions);
    state["pending_question"] = question;
    state["active"] = json!(true);
    state["phase"] = json!("question_pending");
    state["updated_at"] = json!(timestamp);
}

fn answer_pending_question(
    timestamp: &str,
    state: &mut Value,
    prompt: &str,
    payload_file: &Path,
) -> Option<String> {
    let pending = state.get("pending_question")?;
    if pending.get("status").and_then(Value::as_str) != Some("pending") {
        return None;
    }
    let pending_id = pending.get("id").map(value_to_string)?;
    let answer = json!({
        "content": prompt,
        "answered_at": timestamp,
        "payload": payload_file,
    });

    if let Some(questions) = state.get_mut("questions").and_then(Value::as_array_mut) {
        if let Some(existing) = questions.iter_mut().find(|existing| {
            existing.get("id").map(value_to_string) == Some(pending_id.clone())
                && existing.get("status").and_then(Value::as_str) == Some("pending")
        }) {
            existing["status"] = json!("answered");
            existing["answer"] = answer;
        }
    }
    state["pending_question"] = Value::Null;
    state["phase"] = json!("interviewing");
    state["updated_at"] = json!(timestamp);
    Some(pending_id)
}

fn update_terminal_state(
    timestamp: &str,
    state: &mut Value,
    terminal: &TerminalState,
    spec: Option<&PersistedSpec>,
) {
    let terminal_statuses = HashSet::from([
        "approved",
        "crystallized",
        "cancelled",
        "canceled",
        "complete",
        "completed",
        "rejected",
    ]);
    let active = !terminal_statuses.contains(terminal.status.as_str());
    state["active"] = json!(active);
    state["phase"] = json!(terminal.status);
    state["status"] = json!(terminal.status);
    if !terminal.ambiguity.is_empty() {
        state["ambiguity"] = json!(terminal.ambiguity);
    }
    if !terminal.next.is_empty() {
        state["next"] = json!(terminal.next);
    }
    if let Some(plan_id) = &terminal.plan_id {
        state["plan_id"] = json!(plan_id);
    }
    if let Some(spec) = spec {
        state["spec_path"] = json!(spec.path);
        state["spec_sha256"] = json!(spec.sha256);
        state["spec_persisted_at"] = json!(spec.persisted_at);
        state["spec_payload"] = json!(spec.payload);
    }
    if !active {
        state["pending_question"] = Value::Null;
        state["closed_at"] = json!(timestamp);
    }
    state["updated_at"] = json!(timestamp);
}

fn require_ralplan_input_lock(timestamp: &str, state: &mut Value, payload_file: &Path) {
    state["active"] = json!(true);
    state["phase"] = json!("input_lock_required");
    state["status"] = json!("input_lock_required");
    state["requires_input_lock"] = json!(true);
    state["required_input_workflow"] = json!(DEEP_INTERVIEW);
    state["approval_status"] = json!("awaiting_plan");
    state["last_input_lock_payload"] = json!(payload_file);
    state["updated_at"] = json!(timestamp);
}

fn reject_crystallized_without_spec(timestamp: &str, state: &mut Value) {
    state["active"] = json!(true);
    state["phase"] = json!("crystallization_missing_spec");
    state["status"] = json!("crystallization_missing_spec");
    state["pending_question"] = Value::Null;
    state["updated_at"] = json!(timestamp);
}

fn reject_ralplan_without_plan(timestamp: &str, state: &mut Value, plan_id: &str) {
    state["active"] = json!(true);
    state["phase"] = json!("plan_missing");
    state["status"] = json!("plan_missing");
    state["plan_id"] = json!(plan_id);
    state["approval_status"] = json!("blocked");
    state["updated_at"] = json!(timestamp);
}

fn reject_ralplan_input_lock(
    timestamp: &str,
    state: &mut Value,
    plan_id: &str,
    reason: &'static str,
) {
    remove_state_fields(
        state,
        &[
            "plan_gate",
            "plan_path",
            "plan_sha256",
            "plan_persisted_at",
            "plan_payload",
            "input_spec_path",
            "input_spec_sha256",
            "input_spec_persisted_at",
        ],
    );
    state["active"] = json!(true);
    state["phase"] = json!("input_lock_blocked");
    state["status"] = json!("input_lock_blocked");
    state["plan_id"] = json!(plan_id);
    state["requires_input_lock"] = json!(true);
    state["required_input_workflow"] = json!(DEEP_INTERVIEW);
    state["approval_status"] = json!("blocked");
    state["input_lock_status"] = json!(reason);
    state["updated_at"] = json!(timestamp);
}

fn reject_ralplan_without_reviews(timestamp: &str, state: &mut Value, plan_id: &str) {
    state["active"] = json!(true);
    state["phase"] = json!("review_incomplete");
    state["status"] = json!("review_incomplete");
    state["plan_id"] = json!(plan_id);
    state["approval_status"] = json!("blocked");
    state["updated_at"] = json!(timestamp);
}

fn reject_ralplan_handoff_not_ready(
    timestamp: &str,
    state: &mut Value,
    plan_id: &str,
    blocker: &Value,
) {
    state["active"] = json!(true);
    state["phase"] = json!("handoff_not_ready");
    state["status"] = json!("handoff_not_ready");
    state["plan_id"] = json!(plan_id);
    state["approval_status"] = json!("blocked");
    state["blocked_by"] = json!(DEEP_INTERVIEW);
    state["blocked_phase"] = blocker.get("phase").cloned().unwrap_or(Value::Null);
    state["blocked_status"] = blocker.get("status").cloned().unwrap_or(Value::Null);
    state["updated_at"] = json!(timestamp);
}

#[derive(Debug)]
struct MutationSignal {
    kind: &'static str,
    value: String,
}

fn mutation_signal(payload: &Value) -> Option<MutationSignal> {
    let tool_input = payload.get("tool_input").and_then(Value::as_object);
    let command = tool_input
        .and_then(|input| input.get("command").or_else(|| input.get("cmd")))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if mutating_command(command) {
        return Some(MutationSignal {
            kind: "command",
            value: command.to_string(),
        });
    }

    for name in tool_names(payload) {
        if matches!(
            name.as_str(),
            "applypatch" | "edit" | "multiedit" | "notebookedit" | "write"
        ) {
            return Some(MutationSignal {
                kind: "tool",
                value: name,
            });
        }
    }
    None
}

fn tool_names(payload: &Value) -> Vec<String> {
    let mut names = Vec::new();
    collect_tool_names(payload, &mut names);
    if let Some(tool_input) = payload.get("tool_input") {
        collect_tool_names(tool_input, &mut names);
    }
    names
}

fn collect_tool_names(value: &Value, names: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for key in ["tool_name", "toolName", "tool", "name"] {
        match object.get(key) {
            Some(Value::String(name)) => names.push(normalized_tool_name(name)),
            Some(Value::Object(nested)) => {
                if let Some(Value::String(name)) = nested.get("name") {
                    names.push(normalized_tool_name(name));
                }
            }
            _ => {}
        }
    }
}

fn normalized_tool_name(name: &str) -> String {
    name.rsplit('.')
        .next()
        .unwrap_or(name)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn mutating_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    if has_mutating_redirection(command) {
        return true;
    }

    command
        .split([';', '&', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .any(mutating_command_segment)
}

fn has_mutating_redirection(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'>' {
            index += 1;
            continue;
        }

        let mut fd_start = index;
        while fd_start > 0 && bytes[fd_start - 1].is_ascii_digit() {
            fd_start -= 1;
        }
        let fd = &command[fd_start..index];
        let mut target_start = index + 1;
        if bytes.get(target_start) == Some(&b'>') {
            target_start += 1;
        }
        while bytes
            .get(target_start)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            target_start += 1;
        }
        let target_end = if bytes.get(target_start) == Some(&b'&') {
            let mut end = target_start + 1;
            while bytes.get(end).is_some_and(|byte| byte.is_ascii_digit()) {
                end += 1;
            }
            end
        } else {
            command[target_start..]
                .find(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ';' | '|' | '&'))
                .map(|offset| target_start + offset)
                .unwrap_or(command.len())
        };
        let target = &command[target_start..target_end];

        if !is_non_mutating_redirection_target(fd, target) {
            return true;
        }
        index = target_end.max(index + 1);
    }
    false
}

fn is_non_mutating_redirection_target(fd: &str, target: &str) -> bool {
    let stream_redirect = target
        .strip_prefix('&')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()));
    let discard = target == "/dev/null";

    stream_redirect || (discard && matches!(fd, "" | "1" | "2"))
}

fn mutating_command_segment(segment: &str) -> bool {
    let tokens = segment.split_whitespace().collect::<Vec<_>>();
    let Some(first) = tokens.first().copied() else {
        return false;
    };

    if first == "apply_patch" {
        return true;
    }
    if matches!(
        first,
        "rm" | "mv" | "cp" | "mkdir" | "touch" | "chmod" | "chown" | "ln" | "install" | "tee"
    ) {
        return true;
    }
    if first == "git"
        && tokens.get(1).is_some_and(|verb| {
            matches!(
                *verb,
                "add"
                    | "commit"
                    | "push"
                    | "tag"
                    | "checkout"
                    | "switch"
                    | "reset"
                    | "merge"
                    | "rebase"
                    | "restore"
            )
        })
    {
        return true;
    }
    if matches!(first, "npm" | "pnpm" | "yarn" | "bun")
        && tokens
            .get(1)
            .is_some_and(|verb| matches!(*verb, "install" | "add" | "remove" | "update"))
    {
        return true;
    }
    if first == "cargo" && tokens.get(1) == Some(&"fmt") {
        return true;
    }
    if first == "sed" && tokens.get(1).is_some_and(|arg| arg.starts_with("-i")) {
        return true;
    }
    first == "perl" && tokens.get(1).is_some_and(|arg| arg.starts_with("-pi"))
}
