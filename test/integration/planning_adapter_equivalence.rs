use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{json, Value};
use tempfile::tempdir;

fn rpc(project: &Path, request: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_megara"))
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .args(["planning", "rpc", "--project"])
        .arg(project)
        .current_dir(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.as_mut().unwrap(), "{}", request).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.stderr.is_empty(), "stderr={:?}", output.stderr);
    let lines = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "stdout={lines:?}");
    serde_json::from_str(&lines[0]).unwrap()
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpClient {
    fn start(project: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_megara"))
            .env("MEGARA_NO_UPDATE_CHECK", "1")
            .args(["planning", "mcp", "--project"])
            .arg(project)
            .current_dir(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn request(&mut self, request: Value, id: u64) -> Value {
        writeln!(self.stdin, "{request}").unwrap();
        self.stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).unwrap();
            assert!(!line.is_empty(), "MCP server closed before response");
            let response: Value = serde_json::from_str(line.trim_end()).unwrap();
            if response.get("id") == Some(&json!(id)) {
                return response;
            }
        }
    }

    fn finish(mut self) {
        drop(self.stdin);
        let status = self.child.wait().unwrap();
        let mut stderr = Vec::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(status.success(), "MCP status={status:?} stderr={stderr:?}");
        assert!(stderr.is_empty(), "MCP stderr={stderr:?}");
    }
}

fn initialize(client: &mut McpClient) {
    let response = client.request(
        json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params": {
                "protocolVersion":"1999-01-01",
                "capabilities":{},
                "clientInfo":{"name":"transport-fixture","version":"1"}
            }
        }),
        1,
    );
    assert!(response["result"]["protocolVersion"].is_string());
    assert_ne!(response["result"]["protocolVersion"], "1999-01-01");
    writeln!(
        client.stdin,
        "{}",
        json!({
            "jsonrpc":"2.0", "method":"notifications/initialized", "params":{}
        })
    )
    .unwrap();
    client.stdin.flush().unwrap();
}

fn mcp_tool(client: &mut McpClient, id: u64, name: &str, arguments: Value) -> Value {
    let response = client.request(
        json!({
            "jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":name,"arguments":arguments}
        }),
        id,
    );
    response["result"]["structuredContent"].clone()
}

fn start_request(command_id: &str, request_id: &str) -> Value {
    json!({
        "protocol_version":1, "request_id":request_id,
        "operation":"planning.start", "command_id":command_id,
        "params":{"request":"transport equivalence request","title":"same title"}
    })
}

fn evidence_request(session_id: &str, command_id: &str, request_id: &str) -> Value {
    json!({
        "protocol_version":1, "request_id":request_id,
        "operation":"planning.evidence.refresh", "command_id":command_id,
        "session_id":session_id, "expected_revision":1,
        "params":{"citations":[{
            "temp_ref":"main", "path":"src/main.rs", "ranges":[],
            "claim":"transport fixture entry point"
        }]}
    })
}

fn question() -> Value {
    json!({
        "context":"결정 배경입니다.",
        "question":"어떤 결과를 원하시나요?",
        "why_it_matters":"답에 따라 계획이 달라집니다.",
        "technical_terms":[],
        "source_refs":[{"kind":"initial_request","id":"request"}],
        "answer":{"mode":"freeform","freeform_hint":"원하는 결과를 적어 주세요."}
    })
}

fn audit_request(response: &Value, session_id: &str, command_id: &str, request_id: &str) -> Value {
    let work = &response["result"]["state"]["required_model_action"];
    json!({
        "protocol_version":1, "request_id":request_id,
        "operation":"planning.audit.apply", "command_id":command_id,
        "session_id":session_id, "expected_revision":2,
        "params":{"mode":"delta","proposal":{
            "schema":"megara.audit-proposal/v1", "mode":"delta",
            "work_item_id":work["work_item_id"], "base_revision":work["base_revision"],
            "base_domain_revision":work["base_domain_revision"], "input_hash":work["input_hash"],
            "readiness":"continue", "next_question":question(),
            "entity_ops":[], "edge_ops":[], "blocker_ops":[],
            "counterexample_review":null
        }}
    })
}

fn mcp_arguments(request: &Value) -> Value {
    let params = request["params"].clone();
    let mut arguments = params.as_object().cloned().unwrap_or_default();
    if let Some(session_id) = request.get("session_id") {
        arguments.insert("session_id".to_string(), session_id.clone());
    }
    if let Some(expected_revision) = request.get("expected_revision") {
        arguments.insert("expected_revision".to_string(), expected_revision.clone());
    }
    if let Some(command_id) = request.get("command_id") {
        arguments.insert("command_id".to_string(), command_id.clone());
    }
    Value::Object(arguments)
}

fn run_pi_journey(project: &Path) -> (String, Value) {
    let start = rpc(project, start_request("cmd-pi-start", "req-pi-start"));
    assert_eq!(start["ok"], true);
    let session_id = start["session_id"].as_str().unwrap().to_string();
    let evidence = rpc(
        project,
        evidence_request(&session_id, "cmd-pi-evidence", "req-pi-evidence"),
    );
    assert_eq!(evidence["ok"], true);
    let audit = rpc(
        project,
        audit_request(&evidence, &session_id, "cmd-pi-audit", "req-pi-audit"),
    );
    assert_eq!(audit["ok"], true);
    (session_id, audit)
}

fn run_codex_journey(project: &Path) -> (String, Value) {
    let mut client = McpClient::start(project);
    initialize(&mut client);
    let start = mcp_tool(
        &mut client,
        2,
        "planning_start",
        json!({"request":"transport equivalence request","title":"same title","command_id":"cmd-codex-start"}),
    );
    assert_eq!(start["ok"], true);
    let session_id = start["session_id"].as_str().unwrap().to_string();
    let evidence_request =
        evidence_request(&session_id, "cmd-codex-evidence", "req-codex-evidence");
    let evidence = mcp_tool(
        &mut client,
        3,
        "planning_evidence_refresh",
        mcp_arguments(&evidence_request),
    );
    assert_eq!(evidence["ok"], true);
    let audit_request = audit_request(&evidence, &session_id, "cmd-codex-audit", "req-codex-audit");
    let audit = mcp_tool(
        &mut client,
        4,
        "planning_audit_apply",
        mcp_arguments(&audit_request),
    );
    assert_eq!(audit["ok"], true);
    client.finish();
    (session_id, audit)
}

#[test]
fn pi_rpc_and_codex_mcp_transport_preserve_semantic_journey() {
    let project = tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    std::fs::write(project.path().join("src/main.rs"), "fn main() {}\n").unwrap();

    let (pi_session, pi_audit) = run_pi_journey(project.path());
    let (codex_session, codex_audit) = run_codex_journey(project.path());
    assert_eq!(
        pi_audit["result"]["next_action"]["projection"],
        codex_audit["result"]["next_action"]["projection"]
    );

    let store = crate::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let pi_state = store.current(&pi_session).unwrap();
    let codex_state = store.current(&codex_session).unwrap();
    assert_eq!(
        crate::planning::store::normalized_state_hash(&pi_state),
        crate::planning::store::normalized_state_hash(&codex_state)
    );

    let pi_events = store.event_envelopes(&pi_session).unwrap();
    let codex_events = store.event_envelopes(&codex_session).unwrap();
    assert_eq!(
        store
            .diagnostic_semantic_event_sequence(&pi_session)
            .unwrap(),
        store
            .diagnostic_semantic_event_sequence(&codex_session)
            .unwrap()
    );
    assert!(pi_events.iter().all(|event| {
        event.metadata.actor == crate::planning::store::EventActor::Model
            && event.metadata.adapter == crate::planning::store::EventAdapter::Pi
    }));
    assert!(codex_events.iter().all(|event| {
        event.metadata.actor == crate::planning::store::EventActor::Model
            && event.metadata.adapter == crate::planning::store::EventAdapter::CodexMcp
    }));
}
