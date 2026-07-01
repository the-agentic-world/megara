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

#[derive(Debug)]
pub struct HookOptions {
    pub runtime: String,
    pub event: String,
    pub matcher: String,
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

fn append_jsonl(path: &Path, entry: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, entry)?;
    file.write_all(b"\n")?;
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
    let (session_id, workflow_dir, session_file, events_file) = session_paths(state_dir, payload);

    match options.event.as_str() {
        "Stop" => handle_stop(
            timestamp,
            payload,
            payload_file,
            &session_id,
            &workflow_dir,
            &session_file,
            &events_file,
        ),
        "UserPromptSubmit" => handle_user_prompt(
            timestamp,
            payload,
            payload_file,
            &session_file,
            &events_file,
        ),
        "PreToolUse" => handle_pre_tool_use(
            timestamp,
            payload,
            payload_file,
            &session_file,
            &events_file,
        ),
        _ => Ok(0),
    }
}

fn session_paths(state_dir: &Path, payload: &Value) -> (String, PathBuf, PathBuf, PathBuf) {
    let session_id = payload
        .get("session_id")
        .or_else(|| payload.get("thread_id"))
        .or_else(|| payload.get("turn_id"))
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());

    let workflow_dir = if state_dir.file_name().is_some_and(|name| name == "hooks") {
        state_dir
            .parent()
            .unwrap_or(state_dir)
            .join("workflows")
            .join(DEEP_INTERVIEW)
    } else {
        state_dir.join("workflows").join(DEEP_INTERVIEW)
    };
    let session_file = workflow_dir.join(format!("{}.json", safe_part(&session_id)));
    let events_file = workflow_dir.join("events.jsonl");
    (session_id, workflow_dir, session_file, events_file)
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn handle_stop(
    timestamp: &str,
    payload: &Value,
    payload_file: &Path,
    session_id: &str,
    workflow_dir: &Path,
    session_file: &Path,
    events_file: &Path,
) -> Result<i32> {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let terminal = workflow_state_from_text(text);
    let question = if terminal.is_some() {
        None
    } else {
        question_from_text(timestamp, text, payload_file)
    };

    if terminal.is_none() && question.is_none() {
        return Ok(0);
    }

    let mut state =
        load_json(session_file).unwrap_or_else(|| new_state(timestamp, session_id, payload));
    if let Some(terminal) = terminal {
        let spec = persist_crystallized_spec(
            timestamp,
            workflow_dir,
            session_id,
            &terminal,
            text,
            payload_file,
        )?;
        if terminal.status == "crystallized" && spec.is_none() {
            reject_crystallized_without_spec(timestamp, &mut state);
            append_jsonl(
                events_file,
                &json!({
                    "timestamp": timestamp,
                    "event": "spec_missing",
                    "session_id": session_id,
                    "status": terminal.status,
                    "payload": payload_file,
                }),
            )?;
        } else {
            update_terminal_state(timestamp, &mut state, &terminal, spec.as_ref());
            let mut entry = json!({
                "timestamp": timestamp,
                "event": "workflow_state",
                "session_id": session_id,
                "status": terminal.status,
                "payload": payload_file,
            });
            if let Some(spec) = spec {
                entry["spec_path"] = json!(spec.path);
                entry["spec_sha256"] = json!(spec.sha256);
                append_jsonl(
                    events_file,
                    &json!({
                        "timestamp": timestamp,
                        "event": "spec_persisted",
                        "session_id": session_id,
                        "path": spec.path,
                        "sha256": spec.sha256,
                        "payload": payload_file,
                    }),
                )?;
            }
            append_jsonl(events_file, &entry)?;
        }
    }

    if let Some(question) = question {
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
            events_file,
            &json!({
                "timestamp": timestamp,
                "event": "question_pending",
                "session_id": session_id,
                "question_id": question_id,
                "round": round,
                "component": component,
                "dimension": dimension,
                "payload": payload_file,
            }),
        )?;
    }

    write_json_atomic(session_file, &state)?;
    Ok(0)
}

fn handle_user_prompt(
    timestamp: &str,
    payload: &Value,
    payload_file: &Path,
    session_file: &Path,
    events_file: &Path,
) -> Result<i32> {
    let Some(prompt) = payload.get("prompt").and_then(Value::as_str) else {
        return Ok(0);
    };
    if prompt.trim().is_empty() {
        return Ok(0);
    }
    let Some(mut state) = load_json(session_file) else {
        return Ok(0);
    };
    let Some(question_id) = answer_pending_question(timestamp, &mut state, prompt, payload_file)
    else {
        return Ok(0);
    };

    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    write_json_atomic(session_file, &state)?;
    append_jsonl(
        events_file,
        &json!({
            "timestamp": timestamp,
            "event": "question_answered",
            "session_id": session_id,
            "question_id": question_id,
            "payload": payload_file,
        }),
    )?;
    Ok(0)
}

fn handle_pre_tool_use(
    timestamp: &str,
    payload: &Value,
    payload_file: &Path,
    session_file: &Path,
    events_file: &Path,
) -> Result<i32> {
    if env::var("MEGARA_MUTATION_GUARD").unwrap_or_else(|_| "block".to_string()) == "off" {
        return Ok(0);
    }
    let Some(state) = current_active_state(session_file) else {
        return Ok(0);
    };
    let Some(mutation) = mutation_signal(payload) else {
        return Ok(0);
    };

    let session_id = state
        .get("session_id")
        .map(value_to_string)
        .unwrap_or_else(|| "unknown-session".to_string());
    append_jsonl(
        events_file,
        &json!({
            "timestamp": timestamp,
            "event": "mutation_blocked",
            "session_id": session_id,
            "phase": state.get("phase").cloned().unwrap_or(Value::Null),
            "mutation_kind": mutation.kind,
            "mutation_value": mutation.value,
            "payload": payload_file,
        }),
    )?;

    eprintln!(
        "MEGARA mutation guard: deep-interview is active. Answer the pending question or crystallize/cancel the interview before mutating files."
    );
    if env::var("MEGARA_MUTATION_GUARD").unwrap_or_else(|_| "block".to_string()) == "warn" {
        Ok(0)
    } else {
        Ok(42)
    }
}

#[derive(Debug)]
struct Block {
    fields: BTreeMap<String, String>,
    options: Vec<String>,
}

fn parse_block(text: &str, marker: &str) -> Option<Block> {
    if !text.contains(marker) {
        return None;
    }
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim() == marker)
        .map(|index| index + 1)?;

    let mut fields = BTreeMap::new();
    let mut options = Vec::new();
    let mut current_key = String::new();
    let mut saw_field = false;

    for raw in &lines[start..] {
        if raw.trim().is_empty() {
            if saw_field {
                break;
            }
            continue;
        }

        if (raw.starts_with("  - ") || raw.starts_with("    - ")) && current_key == "options" {
            if let Some((_, value)) = raw.split_once("- ") {
                options.push(value.trim().to_string());
            }
            continue;
        }

        let stripped = raw.trim();
        if !stripped.starts_with("- ") {
            if saw_field {
                break;
            }
            continue;
        }

        let Some((key, value)) = stripped[2..].split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase().replace('-', "_");
        current_key = key.clone();
        saw_field = true;
        if key == "options" {
            options.clear();
        } else {
            fields.insert(key, value.trim().to_string());
        }
    }

    saw_field.then_some(Block { fields, options })
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
        "options": block.options.into_iter().filter(|option| !option.is_empty()).collect::<Vec<_>>(),
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
    status: String,
    ambiguity: String,
    next: String,
}

fn workflow_state_from_text(text: &str) -> Option<TerminalState> {
    let block = parse_block(text, "Megara Workflow State:")?;
    if block.fields.get("skill")?.trim() != DEEP_INTERVIEW {
        return None;
    }
    let status = block.fields.get("status")?.trim().to_ascii_lowercase();
    if status.is_empty() {
        return None;
    }
    Some(TerminalState {
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

fn yaml_string(value: impl std::fmt::Display) -> String {
    serde_json::to_string(&value.to_string()).unwrap_or_else(|_| "\"\"".to_string())
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

fn new_state(timestamp: &str, session_id: &str, payload: &Value) -> Value {
    json!({
        "version": 1,
        "skill": DEEP_INTERVIEW,
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
        "crystallized",
        "cancelled",
        "canceled",
        "complete",
        "completed",
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

fn reject_crystallized_without_spec(timestamp: &str, state: &mut Value) {
    state["active"] = json!(true);
    state["phase"] = json!("crystallization_missing_spec");
    state["status"] = json!("crystallization_missing_spec");
    state["pending_question"] = Value::Null;
    state["updated_at"] = json!(timestamp);
}

fn current_active_state(session_file: &Path) -> Option<Value> {
    let state = load_json(session_file)?;
    if state.get("skill").and_then(Value::as_str) != Some(DEEP_INTERVIEW) {
        return None;
    }
    if state.get("active").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    Some(state)
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

fn mutating_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    if command.contains(">>") || command.contains('>') {
        return true;
    }

    command
        .split([';', '&', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .any(mutating_command_segment)
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
