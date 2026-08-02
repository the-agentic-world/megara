use std::io::{self, Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::planning::protocol::{
    decode_jsonl_frame, LogicalRequest, ProtocolError, MAX_JSONL_FRAME_BYTES, PROTOCOL_VERSION,
};
use crate::planning::service::{protocol_error_response, store_error_response, PlanningService};

#[derive(Debug, Args)]
pub struct PlanningArgs {
    #[command(subcommand)]
    pub command: PlanningCommands,
}

#[derive(Debug, Subcommand)]
pub enum PlanningCommands {
    /// Read one JSONL request and write one JSONL response.
    Rpc(PlanningRpcArgs),
    Start(PlanningStartArgs),
    Answer(PlanningAnswerArgs),
    Status(PlanningSessionArgs),
    Current(PlanningSessionArgs),
    List(PlanningListArgs),
    Purge(PlanningPurgeArgs),
}

#[derive(Debug, Args)]
pub struct PlanningRpcArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningStartArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub request: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("answer_input")
        .required(true)
        .args(["text", "read_stdin"])
))]
pub struct PlanningAnswerArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub question: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long, conflicts_with = "read_stdin")]
    pub text: Option<String>,
    #[arg(long = "stdin")]
    pub read_stdin: bool,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningSessionArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct PlanningListArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub phase: Option<PlanningPhaseArg>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum PlanningPhaseArg {
    Interview,
    Specification,
    Planning,
    Complete,
}

impl PlanningPhaseArg {
    fn as_wire(&self) -> &'static str {
        match self {
            Self::Interview => "interview",
            Self::Specification => "specification",
            Self::Planning => "planning",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Args)]
pub struct PlanningPurgeArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub expected_revision: u64,
    #[arg(long)]
    pub confirm: String,
    #[arg(long)]
    pub command_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: PlanningArgs) -> Result<()> {
    match args.command {
        PlanningCommands::Rpc(args) => run_rpc(args),
        PlanningCommands::Start(args) => run_start(args),
        PlanningCommands::Answer(args) => run_answer(args),
        PlanningCommands::Status(args) => run_session("planning.status", args),
        PlanningCommands::Current(args) => run_session("planning.current", args),
        PlanningCommands::List(args) => run_list(args),
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
        let mut text = String::new();
        io::stdin()
            .take((64 * 1024 + 1) as u64)
            .read_to_string(&mut text)?;
        if text.len() > 64 * 1024 {
            return finish_response(
                protocol_error_response(
                    None,
                    Some("planning.answer"),
                    ProtocolError::InvalidRequest("answer exceeds 64 KiB".to_string()),
                ),
                true,
            );
        }
        text
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

fn run_request(
    project: &std::path::Path,
    request: LogicalRequest,
    user_entrypoint: bool,
    json_output: bool,
) -> Result<()> {
    let request_id = request.request_id.clone();
    let operation = request.operation.clone();
    let response = match PlanningService::open_project(project) {
        Ok(mut service) if user_entrypoint => service.handle_user_request(request),
        Ok(mut service) => service.handle_request(request),
        Err(error) => store_error_response(Some(&request_id), Some(&operation), error),
    };
    finish_response(response, json_output)
}

fn finish_response(response: Value, json_output: bool) -> Result<()> {
    let is_error = response.get("ok").and_then(Value::as_bool) == Some(false);
    if json_output || is_error {
        let line = serde_json::to_string(&response)?;
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{line}")?;
        stdout.flush()?;
    } else if let Some(revision) = response.get("revision").and_then(Value::as_u64) {
        let session = response
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("session");
        println!("{session} revision {revision}");
    } else if let Some(sessions) = response["result"]["sessions"].as_array() {
        println!("{} planning session(s)", sessions.len());
    } else {
        println!("planning operation completed");
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
            | "SESSION_PURGED" => 3,
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

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::now_v7())
}
