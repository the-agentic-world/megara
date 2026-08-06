use std::{
    collections::BTreeSet,
    io::Write,
    process::{Command, Stdio},
};

use serde_json::{json, Value};
use tempfile::tempdir;

fn run_mcp(project: &std::path::Path, requests: &[Value]) -> (Vec<Value>, Vec<u8>, i32) {
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
    let input = requests
        .iter()
        .map(|request| format!("{}\n", serde_json::to_string(request).unwrap()))
        .collect::<String>();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (responses, output.stderr, output.status.code().unwrap_or(-1))
}

fn initialize(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "1999-01-01",
            "capabilities": {},
            "clientInfo": {"name": "megara-fixture", "version": "1"}
        }
    })
}

#[test]
fn rmcp_stdio_negotiates_lists_tools_and_calls_planning_service() {
    let project = tempdir().unwrap();
    let (responses, stderr, status) = run_mcp(
        project.path(),
        &[
            initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params": {"name":"planning_start","arguments":{"request":"MCP request","title":"MCP title","command_id":"cmd-mcp-start"}}
            }),
        ],
    );
    assert_eq!(status, 0);
    assert!(stderr.is_empty(), "stderr={stderr:?}");
    assert_eq!(responses.len(), 3);
    assert_ne!(responses[0]["result"]["protocolVersion"], "1999-01-01");
    assert_eq!(
        responses[0]["result"]["instructions"],
        "Megara manages planning state only; use returned work items with the current host model, submit typed proposals, and never infer approval."
    );
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 17);
    let names = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    let expected_names = [
        "planning_start",
        "planning_answer",
        "planning_status",
        "planning_current",
        "planning_list",
        "planning_evidence_refresh",
        "planning_audit_apply",
        "planning_spec_generate",
        "planning_spec_show",
        "planning_spec_approve",
        "planning_spec_revise",
        "planning_plan_generate",
        "planning_plan_show",
        "planning_plan_approve",
        "planning_plan_revise",
        "planning_export",
        "planning_purge",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    assert_eq!(names, expected_names);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(
            !serde_json::to_string(&tool["inputSchema"])
                .unwrap()
                .contains("\"$ref\""),
            "{} must expose a self-contained input schema",
            tool["name"]
        );
    }
    let evidence = tools
        .iter()
        .find(|tool| tool["name"] == "planning_evidence_refresh")
        .unwrap();
    assert_eq!(
        evidence["inputSchema"]["properties"]["citations"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        evidence["inputSchema"]["properties"]["citations"]["items"]["properties"]["ranges"]
            ["items"]["properties"]["start_line"]["minimum"],
        1
    );
    assert_eq!(
        evidence["inputSchema"]["properties"]["citations"]["items"]["properties"]["ranges"]
            ["items"]["properties"]["end_line"]["minimum"],
        1
    );
    let audit = tools
        .iter()
        .find(|tool| tool["name"] == "planning_audit_apply")
        .unwrap();
    let audit_description = audit["description"].as_str().unwrap();
    assert!(audit_description.contains("next_question has no id or prompt field"));
    assert!(audit_description.contains("result.host_adapter"));
    assert_eq!(
        audit["inputSchema"]["properties"]["proposal"]["properties"]["schema"]["const"],
        "megara.audit-proposal/v1"
    );
    let initial_request_source = audit["inputSchema"]["properties"]["proposal"]["properties"]
        ["next_question"]["anyOf"][0]["properties"]["source_refs"]["items"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["properties"]["kind"]["const"] == "initial_request")
        .unwrap();
    assert_eq!(
        initial_request_source["properties"]["id"]["const"],
        "request"
    );
    assert_eq!(
        audit["inputSchema"]["properties"]["proposal"]["properties"]["next_question"]["anyOf"][0]
            ["additionalProperties"],
        false
    );
    assert_eq!(
        audit["inputSchema"]["properties"]["proposal"]["properties"]["next_question"]["anyOf"][0]
            ["properties"]["answer"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        audit["inputSchema"]["properties"]["proposal"]["properties"]["entity_ops"]["items"]
            ["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        13
    );
    let revise = audit["inputSchema"]["properties"]["proposal"]["properties"]["entity_ops"]
        ["items"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["properties"]["op"]["const"] == "revise")
        .unwrap();
    assert_eq!(
        revise["properties"]["body"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    for body in revise["properties"]["body"]["oneOf"].as_array().unwrap() {
        assert_eq!(body["additionalProperties"], false);
        assert!(body["properties"].get("kind").is_none());
    }
    for name in ["planning_spec_generate", "planning_plan_generate"] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        assert_eq!(
            tool["inputSchema"]["properties"]["proposal"]["additionalProperties"],
            false
        );
    }
    for name in expected_names {
        assert!(
            tools.iter().find(|tool| tool["name"] == name).unwrap()["inputSchema"]
                .get("$defs")
                .is_none(),
            "{name} must not carry proposal definitions"
        );
    }
    assert!(
        serde_json::to_vec(tools).unwrap().len() < 256 * 1024,
        "fully inlined tool catalog must remain bounded"
    );
    for name in [
        "planning_spec_approve",
        "planning_plan_approve",
        "planning_purge",
    ] {
        assert_eq!(
            tools.iter().find(|tool| tool["name"] == name).unwrap()["_meta"]["megara"]
                ["approval_mode"],
            "prompt"
        );
    }
    for name in [
        "planning_start",
        "planning_answer",
        "planning_evidence_refresh",
        "planning_audit_apply",
        "planning_spec_generate",
        "planning_spec_approve",
        "planning_spec_revise",
        "planning_plan_generate",
        "planning_plan_approve",
        "planning_plan_revise",
        "planning_export",
        "planning_purge",
    ] {
        assert!(
            tools.iter().find(|tool| tool["name"] == name).unwrap()["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "command_id")
        );
    }
    assert!(tools
        .iter()
        .find(|tool| tool["name"] == "planning_export")
        .unwrap()
        .get("_meta")
        .is_none());
    assert!(tools
        .iter()
        .find(|tool| tool["name"] == "planning_spec_approve")
        .unwrap()
        .get("_meta")
        .is_some());
    let approve = tools
        .iter()
        .find(|tool| tool["name"] == "planning_spec_approve")
        .unwrap();
    assert_eq!(approve["_meta"]["megara"]["approval_mode"], "prompt");
    assert_eq!(approve["annotations"]["destructiveHint"], false);
    assert_eq!(
        tools
            .iter()
            .find(|tool| tool["name"] == "planning_purge")
            .unwrap()["annotations"]["destructiveHint"],
        true
    );
    assert_eq!(
        tools
            .iter()
            .find(|tool| tool["name"] == "planning_export")
            .unwrap()["annotations"]["destructiveHint"],
        true
    );
    assert_eq!(
        tools
            .iter()
            .find(|tool| tool["name"] == "planning_status")
            .unwrap()["annotations"]["readOnlyHint"],
        true
    );
    assert_eq!(
        tools
            .iter()
            .find(|tool| tool["name"] == "planning_start")
            .unwrap()["annotations"]["idempotentHint"],
        false
    );
    let call = &responses[2]["result"];
    let structured = call["structuredContent"].clone();
    assert_eq!(structured["ok"], true);
    assert_eq!(structured["result"]["operation"], "planning.start");
    assert_eq!(structured["result"]["state"]["title"], "MCP title");
    let adapter = &structured["result"]["host_adapter"];
    assert_eq!(adapter["schema"], "megara.codex-host-adapter/v1");
    assert_eq!(adapter["operation"], "planning_audit_apply");
    let template = adapter["arguments_template"].clone();
    let work_item = &structured["result"]["state"]["required_model_action"];
    assert_eq!(template["session_id"], structured["session_id"]);
    assert_eq!(template["expected_revision"], structured["revision"]);
    assert_eq!(
        template["proposal"]["work_item_id"],
        work_item["work_item_id"]
    );
    assert_eq!(
        template["proposal"]["base_revision"],
        work_item["base_revision"]
    );
    assert_eq!(
        template["proposal"]["base_domain_revision"],
        work_item["base_domain_revision"]
    );
    assert_eq!(template["proposal"]["input_hash"], work_item["input_hash"]);
    let question = &template["proposal"]["next_question"];
    assert!(question.get("id").is_none());
    assert!(question.get("prompt").is_none());
    assert_eq!(
        question["source_refs"],
        json!([{"kind":"initial_request","id":"request"}])
    );
    assert_eq!(question["answer"]["mode"], "choice");
    assert!(question["answer"].get("recommendation").is_some());
    assert!(question["answer"]["freeform_hint"].is_string());
    assert!(question["answer"]["choices"]
        .as_array()
        .unwrap()
        .iter()
        .all(|choice| choice["benefits"].is_array() && choice["tradeoffs"].is_array()));
    let (audit_responses, audit_stderr, audit_status) = run_mcp(
        project.path(),
        &[
            initialize(4),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "id":5,
                "method":"tools/call",
                "params":{"name":"planning_audit_apply","arguments":template}
            }),
        ],
    );
    assert_eq!(audit_status, 0);
    assert!(audit_stderr.is_empty(), "stderr={audit_stderr:?}");
    assert_eq!(
        audit_responses[1]["result"]["structuredContent"]["ok"],
        true
    );
    assert_eq!(
        audit_responses[1]["result"]["structuredContent"]["result"]["next_action"]["kind"],
        "question"
    );
    assert!(audit_responses[1]["result"]["structuredContent"]["result"]
        .get("host_adapter")
        .is_none());
    let session = structured["session_id"].as_str().unwrap();
    let store = crate::planning::store::PlanningStore::open_project(project.path()).unwrap();
    let event = store.event_envelopes(session).unwrap().remove(0);
    assert_eq!(
        event.metadata.actor,
        crate::planning::store::EventActor::Model
    );
    assert_eq!(
        event.metadata.adapter,
        crate::planning::store::EventAdapter::CodexMcp
    );
}

#[test]
fn mcp_rejects_transport_actor_injection_with_one_error_response() {
    let project = tempdir().unwrap();
    let (responses, stderr, status) = run_mcp(
        project.path(),
        &[
            initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"planning_start","arguments":{"request":"x","actor":"user"}}}),
        ],
    );
    assert_eq!(status, 0);
    assert!(stderr.is_empty(), "stderr={stderr:?}");
    assert_eq!(responses.len(), 2);
    assert!(responses[1]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("forbidden"));
    let store = crate::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert!(store.list(None).unwrap().is_empty());
}

#[test]
fn mcp_requires_reusable_command_id_for_mutation_retries() {
    let project = tempdir().unwrap();
    let (responses, stderr, status) = run_mcp(
        project.path(),
        &[
            initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"planning_start","arguments":{"request":"x"}}}),
        ],
    );
    assert_eq!(status, 0);
    assert!(stderr.is_empty(), "stderr={stderr:?}");
    assert_eq!(responses.len(), 2);
    assert!(responses[1]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("command_id"));
    let store = crate::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert!(store.list(None).unwrap().is_empty());
}

#[test]
fn mcp_reuses_command_result_and_rejects_same_id_for_changed_request() {
    let project = tempdir().unwrap();
    let (responses, stderr, status) = run_mcp(
        project.path(),
        &[
            initialize(1),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"planning_start","arguments":{"request":"same","command_id":"cmd-replay"}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"planning_start","arguments":{"request":"same","command_id":"cmd-replay"}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"planning_start","arguments":{"request":"changed","command_id":"cmd-replay"}}}),
        ],
    );
    assert_eq!(status, 0);
    assert!(stderr.is_empty(), "stderr={stderr:?}");
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[1]["result"]["structuredContent"]["ok"], true);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["replayed"],
        true
    );
    assert_eq!(
        responses[3]["result"]["structuredContent"]["error"]["code"],
        "COMMAND_ID_REUSE"
    );
    let session_id = responses[1]["result"]["structuredContent"]["session_id"]
        .as_str()
        .unwrap();
    let store = crate::planning::store::PlanningStore::open_project(project.path()).unwrap();
    assert_eq!(store.event_count(session_id).unwrap(), 1);
}
