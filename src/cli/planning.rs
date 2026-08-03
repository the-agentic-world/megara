use std::io::{self, Read, Write};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::planning::protocol::{
    decode_jsonl_frame, EvidenceCitationRequest, LogicalRequest, ProtocolError,
    EVIDENCE_CITATIONS_SCHEMA, MAX_JSONL_FRAME_BYTES, PROTOCOL_VERSION,
};
use crate::planning::service::{protocol_error_response, store_error_response, PlanningService};

#[path = "planning_args.rs"]
mod args;
#[path = "planning_artifacts.rs"]
mod artifacts;
#[path = "planning_input.rs"]
mod input;
#[path = "planning_migration.rs"]
mod migration;
use args::{
    PlanningAnswerArgs, PlanningAuditApplyArgs, PlanningAuditCommands, PlanningEvidenceCommands,
    PlanningEvidenceRefreshArgs, PlanningListArgs, PlanningPlanCommands, PlanningPurgeArgs,
    PlanningRpcArgs, PlanningSpecCommands,
};
pub use args::{PlanningArgs, PlanningCommands, PlanningSessionArgs, PlanningStartArgs};
use input::{read_json_input, read_text_input};

pub fn run(args: PlanningArgs) -> Result<()> {
    match args.command {
        PlanningCommands::Rpc(args) => run_rpc(args),
        PlanningCommands::Mcp(args) => crate::planning::mcp::run(&args.project),
        PlanningCommands::Migrate(args) => migration::run(args),
        PlanningCommands::Start(args) => run_start(args),
        PlanningCommands::Answer(args) => run_answer(args),
        PlanningCommands::Status(args) => run_session("planning.status", args),
        PlanningCommands::Current(args) => run_session("planning.current", args),
        PlanningCommands::List(args) => run_list(args),
        PlanningCommands::Evidence { command } => match command {
            PlanningEvidenceCommands::Refresh(args) => run_evidence_refresh(args),
        },
        PlanningCommands::Audit { command } => match command {
            PlanningAuditCommands::Apply(args) => run_audit_apply(args),
        },
        PlanningCommands::Spec { command } => match command {
            PlanningSpecCommands::Generate(args) => artifacts::run_spec_generate(args),
            PlanningSpecCommands::Show(args) => artifacts::run_spec_show(args),
            PlanningSpecCommands::Approve(args) => artifacts::run_spec_approve(args),
            PlanningSpecCommands::Revise(args) => artifacts::run_spec_revise(args),
        },
        PlanningCommands::Plan { command } => match command {
            PlanningPlanCommands::Generate(args) => artifacts::run_plan_generate(args),
            PlanningPlanCommands::Show(args) => artifacts::run_plan_show(args),
            PlanningPlanCommands::Approve(args) => artifacts::run_plan_approve(args),
            PlanningPlanCommands::Revise(args) => artifacts::run_plan_revise(args),
        },
        PlanningCommands::Export(args) => artifacts::run_export(args),
        PlanningCommands::Purge(args) => run_purge(args),
    }
}

fn run_rpc(args: PlanningRpcArgs) -> Result<()> {
    let mut frame = Vec::new();
    io::stdin()
        .take((MAX_JSONL_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut frame)?;
    let request = match decode_jsonl_frame(&frame) {
        Ok(request) => request,
        Err(error) => {
            return finish_response(protocol_error_response(None, None, error), true);
        }
    };
    run_request(&args.project, request, false, true)
}

fn run_start(args: PlanningStartArgs) -> Result<()> {
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.start".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: None,
            expected_revision: None,
            params: Some(json!({"request":args.request, "title":args.title})),
        },
        true,
        args.json,
    )
}

fn run_answer(args: PlanningAnswerArgs) -> Result<()> {
    let text = if args.read_stdin {
        match read_text_input("-", 64 * 1024) {
            Ok(text) => text,
            Err(error) => {
                return finish_response(
                    protocol_error_response(None, Some("planning.answer"), error),
                    true,
                )
            }
        }
    } else if let Some(text) = args.text {
        text
    } else {
        return Err(anyhow!("provide exactly one of --text or --stdin"));
    };
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.answer".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({"question_id":args.question, "text":text})),
        },
        true,
        args.json,
    )
}

fn run_session(operation: &str, args: PlanningSessionArgs) -> Result<()> {
    let json_output = args.json;
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: operation.to_string(),
            command_id: None,
            session_id: args.session,
            expected_revision: None,
            params: None,
        },
        true,
        json_output,
    )
}

fn run_list(args: PlanningListArgs) -> Result<()> {
    let json_output = args.json;
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.list".to_string(),
            command_id: None,
            session_id: None,
            expected_revision: None,
            params: args
                .phase
                .as_ref()
                .map(|phase| json!({"phase":phase.as_wire()})),
        },
        true,
        json_output,
    )
}

fn run_purge(args: PlanningPurgeArgs) -> Result<()> {
    let json_output = args.json;
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.purge".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({"confirm":args.confirm})),
        },
        true,
        json_output,
    )
}

fn run_evidence_refresh(args: PlanningEvidenceRefreshArgs) -> Result<()> {
    let citation_request =
        match read_json_input(&args.citations, MAX_JSONL_FRAME_BYTES).and_then(|value| {
            serde_json::from_value::<EvidenceCitationRequest>(value).map_err(|error| {
                ProtocolError::InvalidRequest(format!("evidence citations: {error}"))
            })
        }) {
            Ok(request) => request,
            Err(error) => {
                return finish_response(
                    protocol_error_response(None, Some("planning.evidence.refresh"), error),
                    true,
                )
            }
        };
    if citation_request.schema != EVIDENCE_CITATIONS_SCHEMA
        || citation_request.base_revision != args.expected_revision
    {
        return finish_response(
            protocol_error_response(
                None,
                Some("planning.evidence.refresh"),
                ProtocolError::InvalidRequest(
                    "citation schema/base_revision does not match the command".to_string(),
                ),
            ),
            true,
        );
    }
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.evidence.refresh".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({"citations": citation_request.citations})),
        },
        true,
        args.json,
    )
}

fn run_audit_apply(args: PlanningAuditApplyArgs) -> Result<()> {
    let proposal = match read_json_input(&args.proposal, MAX_JSONL_FRAME_BYTES) {
        Ok(Value::Object(object)) => Value::Object(object),
        Ok(_) => {
            return finish_response(
                protocol_error_response(
                    None,
                    Some("planning.audit.apply"),
                    ProtocolError::InvalidRequest("proposal must be a JSON object".to_string()),
                ),
                true,
            )
        }
        Err(error) => {
            return finish_response(
                protocol_error_response(None, Some("planning.audit.apply"), error),
                true,
            )
        }
    };
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.audit.apply".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({"mode": args.mode.as_wire(), "proposal": proposal})),
        },
        true,
        args.json,
    )
}

pub(super) fn run_request(
    project: &std::path::Path,
    request: LogicalRequest,
    user_entrypoint: bool,
    json_output: bool,
) -> Result<()> {
    let response = request_response(project, request, user_entrypoint);
    finish_response(response, json_output)
}

pub(super) fn request_response(
    project: &std::path::Path,
    request: LogicalRequest,
    user_entrypoint: bool,
) -> Value {
    let request_id = request.request_id.clone();
    let operation = request.operation.clone();
    match PlanningService::open_project(project) {
        Ok(mut service) if user_entrypoint => service.handle_user_request(request),
        Ok(mut service) => service.handle_request(request),
        Err(error) => store_error_response(Some(&request_id), Some(&operation), error),
    }
}

pub(super) fn finish_response(response: Value, json_output: bool) -> Result<()> {
    let is_error = response.get("ok").and_then(Value::as_bool) == Some(false);
    if json_output || is_error {
        let line = serde_json::to_string(&response)?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{line}")?;
        stdout.flush()?;
    } else {
        print_human_success(&response);
    }
    if is_error {
        let code = response["error"]["code"].as_str().unwrap_or_default();
        let exit_code = match code {
            "REVISION_CONFLICT"
            | "INVALID_PHASE"
            | "CANDIDATE_STALE"
            | "APPROVAL_BINDING_MISMATCH"
            | "COMMAND_ID_REUSE"
            | "COMMAND_ID_RETIRED"
            | "BLOCKERS_PRESENT"
            | "QUESTION_MISMATCH"
            | "SESSION_PURGED"
            | "EVIDENCE_STALE"
            | "MODEL_ACTION_MISMATCH"
            | "PROPOSAL_BASE_MISMATCH"
            | "CANDIDATE_NOT_FOUND" => 3,
            "DB_BUSY"
            | "DB_CORRUPT"
            | "PROJECTION_DIVERGED"
            | "SCHEMA_UPGRADE_REQUIRED"
            | "SCHEMA_VERSION_UNSUPPORTED"
            | "IO_ERROR" => 5,
            _ => 2,
        };
        std::process::exit(exit_code);
    }
    Ok(())
}

fn print_human_success(response: &Value) {
    if let Some(next_action) = response["result"]["next_action"].as_object() {
        match next_action.get("kind").and_then(Value::as_str) {
            Some("model") => {
                let work_item = &next_action["work_item"];
                let kind = work_item["kind"].as_str().unwrap_or("unknown");
                let work_item_id = work_item["work_item_id"].as_str().unwrap_or("unknown");
                println!("next action: {kind}");
                println!("work item: {work_item_id}");
            }
            Some("question") => {
                let question_id = next_action["question"]["question_id"]
                    .as_str()
                    .unwrap_or("unknown");
                println!("next action: question");
                println!("question: {question_id}");
            }
            _ => println!("next action: unknown"),
        }
    } else if let Some(work_item) = response["result"]["state"]["required_model_action"].as_object()
    {
        let kind = work_item["kind"].as_str().unwrap_or("unknown");
        let work_item_id = work_item["work_item_id"].as_str().unwrap_or("unknown");
        println!("next action: {kind}");
        println!("work item: {work_item_id}");
    } else if let Some(question) = response["result"]["state"]["pending_question"].as_object() {
        let question_id = question["question_id"].as_str().unwrap_or("unknown");
        println!("next action: question");
        println!("question: {question_id}");
    } else if let Some(sessions) = response["result"]["sessions"].as_array() {
        println!("{} planning session(s)", sessions.len());
    } else {
        println!("planning operation completed");
    }
    if let Some(revision) = response.get("revision").and_then(Value::as_u64) {
        let session = response
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("session");
        println!("{session} revision {revision}");
    }
}

pub(super) fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::now_v7())
}
