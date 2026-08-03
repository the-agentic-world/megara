use anyhow::Result;
use serde_json::{json, Value};

use crate::planning::protocol::{
    LogicalRequest, ProtocolError, MAX_JSONL_FRAME_BYTES, PROTOCOL_VERSION,
};
use crate::planning::service::render_candidate_markdown;

use super::args::{
    PlanningCandidateFormat, PlanningCandidateGenerateArgs, PlanningCandidateReviseArgs,
    PlanningCandidateShowArgs, PlanningExportArgs, PlanningPlanApproveArgs,
    PlanningSpecApproveArgs,
};
use super::input::{read_json_input, read_text_input};
use super::{finish_response, new_id, request_response, run_request};

pub(crate) fn run_spec_generate(args: PlanningCandidateGenerateArgs) -> Result<()> {
    run_candidate_generate("planning.spec.generate", args)
}

pub(crate) fn run_plan_generate(args: PlanningCandidateGenerateArgs) -> Result<()> {
    run_candidate_generate("planning.plan.generate", args)
}

fn run_candidate_generate(operation: &str, args: PlanningCandidateGenerateArgs) -> Result<()> {
    let proposal = match read_json_input(&args.proposal, MAX_JSONL_FRAME_BYTES) {
        Ok(Value::Object(object)) => Value::Object(object),
        Ok(_) => {
            return finish_response(
                super::protocol_error_response(
                    None,
                    Some(operation),
                    ProtocolError::InvalidRequest("proposal must be an object".to_string()),
                ),
                true,
            )
        }
        Err(error) => {
            return finish_response(
                super::protocol_error_response(None, Some(operation), error),
                true,
            )
        }
    };
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: operation.to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({
                "proposal": proposal,
                "projection_policy": {"force": args.force}
            })),
        },
        true,
        args.json,
    )
}

pub(crate) fn run_spec_show(args: PlanningCandidateShowArgs) -> Result<()> {
    run_candidate_show("planning.spec.show", args)
}

pub(crate) fn run_plan_show(args: PlanningCandidateShowArgs) -> Result<()> {
    run_candidate_show("planning.plan.show", args)
}

fn run_candidate_show(operation: &str, args: PlanningCandidateShowArgs) -> Result<()> {
    let format = args.format.unwrap_or(PlanningCandidateFormat::Markdown);
    let json_output = args.json;
    let mut params = serde_json::Map::new();
    if let Some(candidate_id) = args.candidate_id {
        params.insert("candidate_id".to_string(), Value::String(candidate_id));
    }
    params.insert(
        "format".to_string(),
        Value::String(format.as_wire().to_string()),
    );
    let response = request_response(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: operation.to_string(),
            command_id: None,
            session_id: args.session,
            expected_revision: None,
            params: (!params.is_empty()).then_some(Value::Object(params)),
        },
        true,
    );
    if response.get("ok").and_then(Value::as_bool) != Some(true) || json_output {
        return finish_response(response, true);
    }
    let text = render_candidate(
        &response["result"]["candidate"],
        &format,
        operation.contains(".plan."),
    );
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn render_candidate(candidate: &Value, format: &PlanningCandidateFormat, is_plan: bool) -> String {
    let content = &candidate["content"];
    if matches!(format, PlanningCandidateFormat::Json) {
        return serde_json::to_string_pretty(content).unwrap_or_else(|_| "{}".to_string());
    }
    render_candidate_markdown(candidate, if is_plan { "plan" } else { "spec" })
}

pub(crate) fn run_spec_approve(args: PlanningSpecApproveArgs) -> Result<()> {
    run_approve(
        "planning.spec.approve",
        args.project,
        args.session,
        args.expected_revision,
        args.candidate,
        args.semantic_hash,
        args.base_domain_revision,
        args.command_id,
        args.json,
        "base_domain_revision",
    )
}

pub(crate) fn run_plan_approve(args: PlanningPlanApproveArgs) -> Result<()> {
    run_approve(
        "planning.plan.approve",
        args.project,
        args.session,
        args.expected_revision,
        args.candidate,
        args.semantic_hash,
        args.base_plan_revision,
        args.command_id,
        args.json,
        "base_plan_revision",
    )
}

#[allow(clippy::too_many_arguments)]
fn run_approve(
    operation: &str,
    project: std::path::PathBuf,
    session: String,
    expected_revision: u64,
    candidate: String,
    semantic_hash: String,
    base_revision: u64,
    command_id: Option<String>,
    json_output: bool,
    base_key: &str,
) -> Result<()> {
    let mut params = serde_json::Map::new();
    params.insert("candidate_id".to_string(), Value::String(candidate));
    params.insert("semantic_hash".to_string(), Value::String(semantic_hash));
    params.insert(base_key.to_string(), json!(base_revision));
    run_request(
        &project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: operation.to_string(),
            command_id: Some(command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(session),
            expected_revision: Some(expected_revision),
            params: Some(Value::Object(params)),
        },
        true,
        json_output,
    )
}

pub(crate) fn run_spec_revise(args: PlanningCandidateReviseArgs) -> Result<()> {
    run_revise("planning.spec.revise", args)
}

pub(crate) fn run_plan_revise(args: PlanningCandidateReviseArgs) -> Result<()> {
    run_revise("planning.plan.revise", args)
}

fn run_revise(operation: &str, args: PlanningCandidateReviseArgs) -> Result<()> {
    let text = if args.read_stdin {
        match read_text_input("-", 64 * 1024) {
            Ok(text) => text,
            Err(error) => {
                return finish_response(
                    super::protocol_error_response(None, Some(operation), error),
                    true,
                )
            }
        }
    } else {
        args.text.unwrap_or_default()
    };
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: operation.to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: Some(args.session),
            expected_revision: Some(args.expected_revision),
            params: Some(json!({"candidate_id":args.candidate, "text":text})),
        },
        true,
        args.json,
    )
}

pub(crate) fn run_export(args: PlanningExportArgs) -> Result<()> {
    run_request(
        &args.project,
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: new_id("req"),
            operation: "planning.export".to_string(),
            command_id: Some(args.command_id.unwrap_or_else(|| new_id("cmd"))),
            session_id: args.session,
            expected_revision: None,
            params: Some(json!({
                "out": args.out,
                "format": args.format.as_wire(),
                "include_transcript": args.include_transcript,
                "force": args.force,
            })),
        },
        true,
        args.json,
    )
}
