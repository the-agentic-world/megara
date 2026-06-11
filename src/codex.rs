use crate::clarification::{self, ClarificationRequest};
use crate::tasks::AgentTask;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexDispatchPath {
    AppServer,
    ExecJson,
    DeepLinkManual,
}

impl CodexDispatchPath {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppServer => "app-server",
            Self::ExecJson => "exec-json",
            Self::DeepLinkManual => "deep-link-manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexCapabilities {
    pub app_server_available: bool,
    pub app_server_thread_start_available: bool,
    pub exec_json_available: bool,
    pub resume_available: bool,
    pub deep_link_available: bool,
}

impl CodexCapabilities {
    pub fn probe() -> Self {
        let app_server_help = command_output(&["app-server", "--help"]);
        let exec_help = command_output(&["exec", "--help"]);
        let app_server_available = app_server_help.is_some();

        Self {
            app_server_available,
            app_server_thread_start_available: app_server_available
                && probe_app_server_thread_start(),
            exec_json_available: exec_help
                .as_deref()
                .is_some_and(|help| help.contains("--json")),
            resume_available: command_output(&["resume", "--help"]).is_some(),
            deep_link_available: true,
        }
    }

    pub fn preferred_dispatch_path(&self) -> CodexDispatchPath {
        if self.app_server_available && self.app_server_thread_start_available {
            CodexDispatchPath::AppServer
        } else if self.exec_json_available {
            CodexDispatchPath::ExecJson
        } else {
            CodexDispatchPath::DeepLinkManual
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexDispatchResult {
    pub path: CodexDispatchPath,
    pub thread_id: Option<String>,
    pub resume_hint: Option<String>,
    pub app_deep_link: Option<String>,
    pub clarification_request: Option<ClarificationRequest>,
}

pub fn dispatch(task: &AgentTask, dry_run: bool) -> Result<CodexDispatchResult> {
    let capabilities = CodexCapabilities::probe();
    let path = capabilities.preferred_dispatch_path();

    if dry_run {
        return Ok(dispatch_result(path, None, None, task));
    }

    match path {
        CodexDispatchPath::AppServer => dispatch_app_server(task),
        CodexDispatchPath::ExecJson => dispatch_exec_json(task),
        CodexDispatchPath::DeepLinkManual => Ok(dispatch_result(path, None, None, task)),
    }
}

fn dispatch_app_server(task: &AgentTask) -> Result<CodexDispatchResult> {
    let mut rpc = AppServerJsonRpc::spawn()?;
    rpc.initialize()?;
    let thread_start = rpc.thread_start(task, false)?;
    let thread_id = thread_id_from_thread_start(&thread_start)?;
    let turn_start = rpc.turn_start(&thread_id, task)?;
    let turn_id = turn_id_from_turn_start(&turn_start)?;
    let transcript = rpc.read_until_turn_completed(&thread_id, &turn_id)?;
    rpc.shutdown();

    let clarification_request = clarification::parse_clarification_request(&transcript)?;
    Ok(dispatch_result(
        CodexDispatchPath::AppServer,
        Some(thread_id),
        clarification_request,
        task,
    ))
}

fn dispatch_exec_json(task: &AgentTask) -> Result<CodexDispatchResult> {
    let mut child = Command::new("codex")
        .arg("exec")
        .arg("--json")
        .arg("--cd")
        .arg(&task.repo_path)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start codex exec --json")?;

    let mut stdin = child.stdin.take().context("failed to open codex stdin")?;
    stdin
        .write_all(task.prompt.as_bytes())
        .context("failed to write Codex task prompt")?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("failed to wait for codex exec --json")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("codex exec --json failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let thread_id = extract_thread_id(&stdout);
    let clarification_request = clarification::parse_clarification_request(&stdout)?;
    Ok(dispatch_result(
        CodexDispatchPath::ExecJson,
        thread_id,
        clarification_request,
        task,
    ))
}

fn dispatch_result(
    path: CodexDispatchPath,
    thread_id: Option<String>,
    clarification_request: Option<ClarificationRequest>,
    task: &AgentTask,
) -> CodexDispatchResult {
    let resume_hint = thread_id
        .as_ref()
        .map(|thread_id| format!("codex resume {thread_id}"));
    let app_deep_link = thread_id
        .as_ref()
        .map(|thread_id| format!("codex://threads/{thread_id}"))
        .or_else(|| {
            if path == CodexDispatchPath::DeepLinkManual {
                Some(manual_deep_link(task))
            } else {
                None
            }
        });

    CodexDispatchResult {
        path,
        thread_id,
        resume_hint,
        app_deep_link,
        clarification_request,
    }
}

fn manual_deep_link(task: &AgentTask) -> String {
    format!(
        "codex://threads/new?prompt={}&path={}",
        urlencoding::encode(&task.prompt),
        urlencoding::encode(&task.repo_path.to_string_lossy())
    )
}

fn command_output(args: &[&str]) -> Option<String> {
    let output = Command::new("codex").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    Some(combined)
}

fn probe_app_server_thread_start() -> bool {
    let Ok(cwd) = std::env::current_dir() else {
        return false;
    };
    let probe_task = AgentTask {
        queue_item_id: 0,
        issue_url: String::new(),
        repo_path: cwd,
        prompt: String::new(),
    };

    let result = (|| -> Result<()> {
        let mut rpc = AppServerJsonRpc::spawn()?;
        rpc.initialize()?;
        rpc.thread_start(&probe_task, true)?;
        rpc.shutdown();
        Ok(())
    })();

    result.is_ok()
}

struct AppServerJsonRpc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl AppServerJsonRpc {
    fn spawn() -> Result<Self> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to start codex app-server --stdio")?;
        let stdin = child
            .stdin
            .take()
            .context("failed to open app-server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to open app-server stdout")?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn initialize(&mut self) -> Result<Value> {
        self.request(
            1,
            "initialize",
            json!({
                "clientInfo": {
                    "name": "sisyphus",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        )
    }

    fn thread_start(&mut self, task: &AgentTask, ephemeral: bool) -> Result<Value> {
        self.request(
            2,
            "thread/start",
            json!({
                "cwd": task.repo_path,
                "runtimeWorkspaceRoots": [task.repo_path],
                "ephemeral": ephemeral
            }),
        )
    }

    fn turn_start(&mut self, thread_id: &str, task: &AgentTask) -> Result<Value> {
        self.request(
            3,
            "turn/start",
            json!({
                "threadId": thread_id,
                "cwd": task.repo_path,
                "runtimeWorkspaceRoots": [task.repo_path],
                "input": [
                    {
                        "type": "text",
                        "text": task.prompt
                    }
                ]
            }),
        )
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        writeln!(self.stdin, "{request}")
            .with_context(|| format!("failed to write app-server request {method}"))?;
        self.stdin
            .flush()
            .with_context(|| format!("failed to flush app-server request {method}"))?;

        loop {
            let value = self.read_message()?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }

            if let Some(error) = value.get("error") {
                bail!("app-server request {method} failed: {error}");
            }

            return value
                .get("result")
                .cloned()
                .with_context(|| format!("app-server response {method} missing result"));
        }
    }

    fn read_until_turn_completed(&mut self, thread_id: &str, turn_id: &str) -> Result<String> {
        let mut transcript = String::new();
        let mut assistant_text = String::new();

        loop {
            let value = self.read_message()?;
            transcript.push_str(&value.to_string());
            transcript.push('\n');

            if message_method(&value) == Some("item/agentMessage/delta")
                && message_thread_id(&value) == Some(thread_id)
                && message_turn_id(&value) == Some(turn_id)
                && let Some(delta) = value
                    .get("params")
                    .and_then(|params| params.get("delta"))
                    .and_then(Value::as_str)
            {
                assistant_text.push_str(delta);
            }

            if message_method(&value) == Some("turn/completed")
                && message_thread_id(&value) == Some(thread_id)
                && completed_turn_id(&value) == Some(turn_id)
            {
                transcript.push_str(&assistant_text);
                transcript.push('\n');
                return Ok(transcript);
            }

            if message_method(&value) == Some("thread/status/changed")
                && message_thread_id(&value) == Some(thread_id)
                && thread_status_type(&value) == Some("idle")
            {
                transcript.push_str(&assistant_text);
                transcript.push('\n');
                return Ok(transcript);
            }
        }
    }

    fn read_message(&mut self) -> Result<Value> {
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .context("failed to read app-server response")?;
        if read == 0 {
            bail!("app-server closed stdout before response");
        }

        serde_json::from_str(line.trim_end()).context("failed to parse app-server JSON message")
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn thread_id_from_thread_start(result: &Value) -> Result<String> {
    result
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("app-server thread/start response missing thread.id")
}

fn turn_id_from_turn_start(result: &Value) -> Result<String> {
    result
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .context("app-server turn/start response missing turn.id")
}

fn message_method(value: &Value) -> Option<&str> {
    value.get("method").and_then(Value::as_str)
}

fn message_thread_id(value: &Value) -> Option<&str> {
    value
        .get("params")
        .and_then(|params| params.get("threadId"))
        .and_then(Value::as_str)
}

fn message_turn_id(value: &Value) -> Option<&str> {
    value
        .get("params")
        .and_then(|params| params.get("turnId"))
        .and_then(Value::as_str)
}

fn completed_turn_id(value: &Value) -> Option<&str> {
    value
        .get("params")
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
}

fn thread_status_type(value: &Value) -> Option<&str> {
    value
        .get("params")
        .and_then(|params| params.get("status"))
        .and_then(|status| status.get("type"))
        .and_then(Value::as_str)
}

fn extract_thread_id(jsonl: &str) -> Option<String> {
    for line in jsonl.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| value.get("event").and_then(Value::as_str));
        if (event_type == Some("thread.started") || event_type == Some("thread_started"))
            && let Some(thread_id) = find_string_key(&value, "thread_id")
                .or_else(|| find_string_key(&value, "threadId"))
                .or_else(|| {
                    value
                        .get("thread")
                        .and_then(|thread| find_string_key(thread, "id"))
                })
        {
            return Some(thread_id);
        }
    }

    None
}

fn find_string_key(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_str) {
                return Some(found.to_string());
            }

            map.values()
                .find_map(|nested_value| find_string_key(nested_value, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested_value| find_string_key(nested_value, key)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_exec_json_until_app_server_thread_start_is_supported() {
        let capabilities = CodexCapabilities {
            app_server_available: true,
            app_server_thread_start_available: false,
            exec_json_available: true,
            resume_available: true,
            deep_link_available: true,
        };

        assert_eq!(
            capabilities.preferred_dispatch_path(),
            CodexDispatchPath::ExecJson
        );
    }

    #[test]
    fn prefers_app_server_when_thread_start_is_supported() {
        let capabilities = CodexCapabilities {
            app_server_available: true,
            app_server_thread_start_available: true,
            exec_json_available: true,
            resume_available: true,
            deep_link_available: true,
        };

        assert_eq!(
            capabilities.preferred_dispatch_path(),
            CodexDispatchPath::AppServer
        );
    }

    #[test]
    fn extracts_thread_id_from_jsonl() {
        let jsonl = r#"{"type":"thread.started","thread":{"id":"thread-1"}}"#;
        assert_eq!(extract_thread_id(jsonl), Some("thread-1".to_string()));
    }

    #[test]
    fn extracts_thread_id_from_app_server_thread_start() {
        let result = json!({
            "thread": {
                "id": "thread-1",
                "sessionId": "session-1"
            }
        });

        assert_eq!(thread_id_from_thread_start(&result).unwrap(), "thread-1");
    }

    #[test]
    fn extracts_turn_id_from_app_server_turn_start() {
        let result = json!({
            "turn": {
                "id": "turn-1"
            }
        });

        assert_eq!(turn_id_from_turn_start(&result).unwrap(), "turn-1");
    }

    #[test]
    fn recognizes_app_server_turn_completed_notification() {
        let value = json!({
            "method": "turn/completed",
            "params": {
                "threadId": "thread-1",
                "turn": {
                    "id": "turn-1"
                }
            }
        });

        assert_eq!(message_method(&value), Some("turn/completed"));
        assert_eq!(message_thread_id(&value), Some("thread-1"));
        assert_eq!(completed_turn_id(&value), Some("turn-1"));
    }

    #[test]
    fn recognizes_app_server_thread_idle_notification() {
        let value = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {
                    "type": "idle"
                }
            }
        });

        assert_eq!(message_method(&value), Some("thread/status/changed"));
        assert_eq!(message_thread_id(&value), Some("thread-1"));
        assert_eq!(thread_status_type(&value), Some("idle"));
    }
}
