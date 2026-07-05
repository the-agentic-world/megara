#![allow(dead_code)]

#[path = "../../src/cli.rs"]
mod cli;
#[path = "../../src/doctor.rs"]
mod doctor;
#[path = "../../src/hook.rs"]
mod hook;
#[path = "../../src/installer.rs"]
mod installer;
#[path = "../../src/paths.rs"]
mod paths;
#[path = "../../src/targets.rs"]
mod targets;
#[path = "../../src/templates.rs"]
mod templates;
#[path = "../../src/ui.rs"]
mod ui;
#[path = "../../src/ultragoal.rs"]
mod ultragoal;
#[path = "../../src/writer.rs"]
mod writer;

pub(crate) use hook::codex_plan::{
    is_deep_interview_start_prompt, is_plan_settings_notification, plan_collaboration_mode,
    prompt_starts_with_plan, thread_id_from_payload, thread_settings_update_payload,
};
pub(crate) use hook::fsutil::append_jsonl;
pub(crate) use hook::mutation::mutating_command;
pub(crate) use hook::parser::{
    approval_gate_from_text, parse_blocks, text_before_first_workflow_block,
};
pub(crate) use hook::runtime_input::{effective_prompt_text, runtime_context, RuntimeSurface};
pub(crate) use installer::{PlannedFile, MANAGED_MARKER};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::{fs, path::Path};
pub(crate) use ultragoal::*;
pub(crate) use writer::*;

#[path = "hook.rs"]
mod hook_tests;
#[path = "ultragoal.rs"]
mod ultragoal_tests;
#[path = "writer.rs"]
mod writer_tests;
