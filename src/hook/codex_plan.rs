use std::{
    env,
    fs::File,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use super::{state_paths::transcript_session_id, HookOptions};

pub(crate) const PLAN_MODE_BLOCK_REASON: &str =
    "Codex Plan mode를 확인하거나 자동 활성화하지 못했습니다. Plan mode를 활성화한 상태에서 같은 deep-interview 요청을 다시 보내주세요.";

pub(crate) fn deep_interview_plan_mode_block_reason(
    options: &HookOptions,
    payload: &Value,
    prompt: &str,
) -> Result<Option<String>> {
    if !options.runtime.eq_ignore_ascii_case("codex") {
        return Ok(None);
    }
    if !is_deep_interview_start_prompt(prompt) {
        return Ok(None);
    }
    if payload_reports_plan_mode(payload) {
        return Ok(None);
    }

    match activate_plan_mode(payload) {
        Ok(()) => Ok(None),
        Err(_) => Ok(Some(PLAN_MODE_BLOCK_REASON.to_string())),
    }
}

pub(crate) fn is_deep_interview_start_prompt(prompt: &str) -> bool {
    let prompt = prompt_after_optional_plan_prefix(prompt);
    let lower = prompt.trim_start().to_ascii_lowercase();
    lower.starts_with("$deep-interview") || lower.starts_with("[$deep-interview]")
}

fn prompt_after_optional_plan_prefix(prompt: &str) -> &str {
    plan_prefixed_tail(prompt).unwrap_or(prompt)
}

fn plan_prefixed_tail(prompt: &str) -> Option<&str> {
    let trimmed = prompt.trim_start();
    if trimmed.len() < 5 {
        return None;
    }
    let (prefix, rest) = trimmed.split_at(5);
    if !prefix.eq_ignore_ascii_case("/plan") {
        return None;
    }
    let Some(first) = rest.chars().next() else {
        return Some(rest);
    };
    if first.is_whitespace() || first == '$' || first == '[' {
        return Some(rest.trim_start());
    }
    None
}

pub(crate) fn thread_id_from_payload(payload: &Value) -> Option<String> {
    string_field(payload, "thread_id")
        .or_else(|| transcript_session_id(payload))
        .or_else(|| string_field(payload, "session_id"))
}

pub(crate) fn plan_collaboration_mode(
    list_result: &Value,
    fallback_model: Option<&str>,
) -> Result<Value> {
    let plan = collaboration_modes(list_result)
        .iter()
        .find(|mode| {
            mode.get("mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
        })
        .cloned()
        .ok_or_else(|| anyhow!("Codex Plan collaboration mode is unavailable"))?;

    let settings = plan.get("settings").cloned().unwrap_or_else(|| json!({}));
    let model = plan
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            settings
                .get("model")
                .and_then(Value::as_str)
                .filter(|model| !model.trim().is_empty())
                .map(str::to_string)
        })
        .or_else(|| fallback_model.map(str::to_string))
        .ok_or_else(|| anyhow!("Codex Plan collaboration mode did not include a model"))?;
    let reasoning_effort = plan
        .get("reasoning_effort")
        .cloned()
        .or_else(|| settings.get("reasoning_effort").cloned())
        .unwrap_or(Value::Null);

    Ok(json!({
        "mode": "plan",
        "settings": {
            "model": model,
            "reasoning_effort": reasoning_effort,
            "developer_instructions": Value::Null,
        }
    }))
}

pub(crate) fn thread_settings_update_payload(thread_id: &str, collaboration_mode: Value) -> Value {
    json!({
        "threadId": thread_id,
        "collaborationMode": collaboration_mode,
    })
}

pub(crate) fn is_plan_settings_notification(message: &Value, thread_id: &str) -> bool {
    message
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|method| method == "thread/settings/updated")
        && notification_thread_id(message).is_some_and(|candidate| candidate == thread_id)
        && notification_collaboration_mode(message)
            .and_then(Value::as_str)
            .is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
}

fn activate_plan_mode(payload: &Value) -> Result<()> {
    let thread_id =
        thread_id_from_payload(payload).ok_or_else(|| anyhow!("Codex thread id is unavailable"))?;
    let fallback_model = payload
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty());
    let timeout = app_server_timeout();
    let deadline = Deadline::new(timeout);
    let mut child = spawn_app_server_proxy()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Codex app-server proxy stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Codex app-server proxy stdout unavailable"))?;
    let receiver = spawn_stdout_reader(stdout);

    let result = (|| -> Result<()> {
        write_jsonl(
            &mut stdin,
            &json!({
                "id": 1,
                "method": "initialize",
                "params": {
                    "clientInfo": {
                        "name": "megara",
                        "title": "Megara",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "capabilities": {
                        "experimentalApi": true,
                        "requestAttestation": false,
                    }
                }
            }),
        )?;
        wait_for_response(&receiver, 1, &deadline)?;
        write_jsonl(
            &mut stdin,
            &json!({
                "method": "initialized",
                "params": {},
            }),
        )?;
        write_jsonl(
            &mut stdin,
            &json!({
                "id": 2,
                "method": "collaborationMode/list",
                "params": {},
            }),
        )?;
        let list_result = wait_for_response(&receiver, 2, &deadline)?;
        let collaboration_mode = plan_collaboration_mode(&list_result, fallback_model)?;
        write_jsonl(
            &mut stdin,
            &json!({
                "id": 3,
                "method": "thread/settings/update",
                "params": thread_settings_update_payload(&thread_id, collaboration_mode),
            }),
        )?;
        wait_for_update_confirmation(&receiver, &thread_id, &deadline)
    })();

    drop(stdin);
    cleanup_child(&mut child);
    result
}

pub(crate) fn payload_reports_plan_mode(payload: &Value) -> bool {
    value_reports_plan_mode(payload) || transcript_reports_plan_mode(payload)
}

fn value_reports_plan_mode(value: &Value) -> bool {
    string_field(value, "permission_mode").is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
        || string_field(value, "collaboration_mode_kind")
            .is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
        || collaboration_mode_value(value.get("collaborationMode"))
        || collaboration_mode_value(value.get("collaboration_mode"))
}

fn collaboration_mode_value(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if value
        .as_str()
        .is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
    {
        return true;
    }
    value
        .get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.eq_ignore_ascii_case("plan"))
}

fn transcript_reports_plan_mode(payload: &Value) -> bool {
    let Some(turn_id) = string_field(payload, "turn_id") else {
        return false;
    };
    let Some(transcript_path) = payload.get("transcript_path").and_then(Value::as_str) else {
        return false;
    };
    let Ok(file) = File::open(transcript_path) else {
        return false;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains(&turn_id) {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if transcript_record_reports_plan_mode(&record, &turn_id) {
            return true;
        }
    }
    false
}

fn transcript_record_reports_plan_mode(record: &Value, turn_id: &str) -> bool {
    let Some(payload) = record.get("payload") else {
        return false;
    };
    payload
        .get("turn_id")
        .and_then(Value::as_str)
        .is_some_and(|candidate| candidate == turn_id)
        && value_reports_plan_mode(payload)
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    let value = payload.get(key)?;
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    let text = text.trim();
    (!text.is_empty() && text != "null").then(|| text.to_string())
}

fn collaboration_modes(list_result: &Value) -> Vec<Value> {
    if let Some(data) = list_result.get("data").and_then(Value::as_array) {
        return data.clone();
    }
    if let Some(modes) = list_result
        .get("collaborationModes")
        .and_then(Value::as_array)
    {
        return modes.clone();
    }
    if let Some(modes) = list_result
        .get("collaboration_modes")
        .and_then(Value::as_array)
    {
        return modes.clone();
    }
    list_result.as_array().cloned().unwrap_or_default()
}

fn notification_thread_id(message: &Value) -> Option<&str> {
    let params = message.get("params")?;
    params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .and_then(Value::as_str)
}

fn notification_collaboration_mode(message: &Value) -> Option<&Value> {
    let params = message.get("params")?;
    params
        .get("threadSettings")
        .or_else(|| params.get("thread_settings"))
        .and_then(|settings| {
            settings
                .get("collaborationMode")
                .or_else(|| settings.get("collaboration_mode"))
        })
        .and_then(|mode| mode.get("mode"))
}

fn spawn_app_server_proxy() -> Result<Child> {
    let mut command = if let Ok(proxy) = env::var("MEGARA_CODEX_APP_SERVER_PROXY") {
        Command::new(proxy)
    } else {
        let mut command = Command::new("codex");
        command.arg("app-server").arg("proxy");
        command
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn Codex app-server proxy")
}

fn spawn_stdout_reader(stdout: std::process::ChildStdout) -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = sender.send(value);
            }
        }
    });
    receiver
}

fn write_jsonl(stdin: &mut std::process::ChildStdin, message: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, message)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn wait_for_response(receiver: &Receiver<Value>, id: u64, deadline: &Deadline) -> Result<Value> {
    loop {
        let message = recv_message(receiver, deadline)?;
        if message_id(&message) == Some(id) {
            return response_result(message);
        }
    }
}

fn wait_for_update_confirmation(
    receiver: &Receiver<Value>,
    thread_id: &str,
    deadline: &Deadline,
) -> Result<()> {
    let mut update_response_seen = false;
    let mut update_notification_seen = false;
    while !(update_response_seen && update_notification_seen) {
        let message = recv_message(receiver, deadline)?;
        if message_id(&message) == Some(3) {
            response_result(message)?;
            update_response_seen = true;
        } else if is_plan_settings_notification(&message, thread_id) {
            update_notification_seen = true;
        }
    }
    Ok(())
}

fn recv_message(receiver: &Receiver<Value>, deadline: &Deadline) -> Result<Value> {
    match receiver.recv_timeout(deadline.remaining()?) {
        Ok(message) => Ok(message),
        Err(RecvTimeoutError::Timeout) => bail!("Codex app-server proxy timed out"),
        Err(RecvTimeoutError::Disconnected) => bail!("Codex app-server proxy disconnected"),
    }
}

fn message_id(message: &Value) -> Option<u64> {
    message
        .get("id")
        .and_then(Value::as_u64)
        .or_else(|| message.get("id").and_then(Value::as_str)?.parse().ok())
}

fn response_result(message: Value) -> Result<Value> {
    if let Some(error) = message.get("error") {
        bail!("Codex app-server error: {error}");
    }
    Ok(message.get("result").cloned().unwrap_or(Value::Null))
}

fn app_server_timeout() -> Duration {
    env::var("MEGARA_CODEX_PLAN_MODE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|millis| Duration::from_millis(millis.max(25)))
        .unwrap_or_else(|| Duration::from_millis(1_500))
}

fn cleanup_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct Deadline {
    started: Instant,
    duration: Duration,
}

impl Deadline {
    fn new(duration: Duration) -> Self {
        Self {
            started: Instant::now(),
            duration,
        }
    }

    fn remaining(&self) -> Result<Duration> {
        let elapsed = self.started.elapsed();
        if elapsed >= self.duration {
            bail!("Codex app-server proxy timed out");
        }
        Ok(self.duration - elapsed)
    }
}
