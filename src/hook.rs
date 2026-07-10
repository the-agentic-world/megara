use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::cli::HookArgs;

#[path = "hook/artifacts.rs"]
mod artifacts;
#[path = "hook/conversation.rs"]
mod conversation;
#[path = "hook/deep_interview_milestone.rs"]
mod deep_interview_milestone;
#[path = "hook/deep_interview_reassessment.rs"]
mod deep_interview_reassessment;
#[path = "hook/dispatch.rs"]
mod dispatch;
#[path = "hook/fsutil.rs"]
pub(crate) mod fsutil;
#[path = "hook/git_guard.rs"]
mod git_guard;
#[path = "hook/mutation.rs"]
pub(crate) mod mutation;
#[path = "hook/parser.rs"]
pub(crate) mod parser;
#[path = "hook/pre_tool.rs"]
mod pre_tool;
#[path = "hook/ralplan_approval.rs"]
mod ralplan_approval;
#[path = "hook/ralplan_context.rs"]
mod ralplan_context;
#[path = "hook/ralplan_input.rs"]
mod ralplan_input;
#[path = "hook/ralplan_prompt.rs"]
mod ralplan_prompt;
#[path = "hook/ralplan_reviews.rs"]
mod ralplan_reviews;
#[path = "hook/ralplan_state.rs"]
mod ralplan_state;
#[path = "hook/runtime_input.rs"]
pub(crate) mod runtime_input;
#[path = "hook/session_alias.rs"]
mod session_alias;
#[path = "hook/state.rs"]
mod state;
#[path = "hook/state_fields.rs"]
mod state_fields;
#[path = "hook/state_paths.rs"]
pub(crate) mod state_paths;
#[path = "hook/stop.rs"]
mod stop;
#[path = "hook/subagent.rs"]
mod subagent;
#[path = "hook/subagent_gate.rs"]
mod subagent_gate;
#[path = "hook/team.rs"]
mod team;
#[path = "hook/terminal.rs"]
mod terminal;
#[path = "hook/user_prompt.rs"]
mod user_prompt;

use artifacts::{
    has_visible_crystallized_spec, persist_crystallized_spec, persist_pending_plan,
    persist_ralplan_review,
};
use fsutil::{append_jsonl, load_json, write_json_atomic};
use mutation::{mutation_signal, protected_workflow_state_mutation};
use parser::{
    approval_gate_from_text, parse_block, parse_blocks, plan_gate_from_text, question_from_text,
    review_passes_from_text, workflow_state_from_text, Block, TerminalState,
};
use session_alias::reconcile_session_aliases;
use state::{
    answer_pending_question, mark_ralplan_input_lock_ready, new_state,
    reject_crystallized_without_spec, reject_ralplan_handoff_not_ready, reject_ralplan_input_lock,
    reject_ralplan_without_plan, reject_ralplan_without_reviews, require_ralplan_input_lock,
    update_terminal_state, upsert_question,
};
use state_paths::{
    canonical_session_id, safe_part, scoped_state_dir, timestamp, unique_payload_path,
    value_to_string, workflow_paths, WorkflowPaths,
};

const DEEP_INTERVIEW: &str = "deep-interview";
const RALPLAN: &str = "ralplan";
const ULTRAGOAL: &str = "ultragoal";
const TEAM: &str = "team";
const WORKFLOWS: &[&str] = &[DEEP_INTERVIEW, RALPLAN, ULTRAGOAL, TEAM];
const MUTATION_GUARD_WORKFLOWS: &[&str] = &[DEEP_INTERVIEW, RALPLAN, ULTRAGOAL];

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
    let runtime_context = runtime_input::runtime_context(&payload);
    let has_effective_user_prompt = runtime_context.effective_prompt.is_some();

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

    let mut event = json!({
        "timestamp": timestamp,
        "runtime": options.runtime,
        "event": options.event,
        "matcher": options.matcher,
        "surface": runtime_context.surface.as_str(),
        "payload": payload_file,
        "last_payload": last_payload_file,
        "payload_bytes": payload_bytes,
    });
    if let Some(source) = runtime_context.transcript_source {
        event["transcript_source"] = json!(source);
    }
    if let Some(thread_source) = runtime_context.transcript_thread_source {
        event["transcript_thread_source"] = json!(thread_source);
    }
    if let Some(originator) = runtime_context.transcript_originator {
        event["transcript_originator"] = json!(originator);
    }
    append_jsonl(&state_dir.join("events.jsonl"), &event)?;

    conversation::record_conversation_event(
        &state_dir,
        &timestamp,
        &options,
        &payload,
        &payload_file,
        payload_bytes,
    )?;
    if options.event == "UserPromptSubmit" && has_effective_user_prompt {
        git_guard::capture_baseline_if_absent(
            &timestamp,
            &state_dir,
            &payload,
            &payload_file,
            "user_prompt",
        )?;
    }
    if options.event == "PreToolUse" {
        if let Some(reason) = git_guard::block_unsafe_staging_if_needed(
            &timestamp,
            &state_dir,
            &options,
            &payload,
            &payload_file,
        )? {
            eprintln!("{reason}");
            return Ok(42);
        }
        if mutation_signal(&payload).is_some() {
            git_guard::capture_baseline_if_absent(
                &timestamp,
                &state_dir,
                &payload,
                &payload_file,
                "pre_tool",
            )?;
        }
    }
    dispatch::run_workflow_event(&state_dir, &timestamp, &options, &payload, &payload_file)
}
