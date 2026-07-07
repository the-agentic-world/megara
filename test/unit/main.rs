#![allow(dead_code)]

#[path = "../../src/cli.rs"]
mod cli;
#[path = "../../src/docs.rs"]
mod docs;
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
#[path = "../../src/tui.rs"]
mod tui;
#[path = "../../src/ui.rs"]
mod ui;
#[path = "../../src/ultragoal.rs"]
mod ultragoal;
#[path = "../../src/update.rs"]
mod update;
#[path = "../../src/writer.rs"]
mod writer;

pub(crate) use hook::fsutil::append_jsonl;
pub(crate) use hook::mutation::mutating_command;
pub(crate) use hook::parser::{
    approval_gate_from_text, parse_blocks, text_before_first_workflow_block,
};
pub(crate) use hook::runtime_input::{
    assistant_message_from_payload, effective_prompt_text, runtime_context, RuntimeSurface,
};
pub(crate) use hook::state_paths::workflow_paths;
pub(crate) use installer::{PlannedFile, MANAGED_MARKER};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::{fs, path::Path};
pub(crate) use ultragoal::*;
pub(crate) use writer::*;

#[path = "docs.rs"]
mod docs_tests;
#[path = "hook.rs"]
mod hook_tests;
#[path = "tui.rs"]
mod tui_tests;
#[path = "ultragoal.rs"]
mod ultragoal_tests;
#[path = "update.rs"]
mod update_tests;
#[path = "writer.rs"]
mod writer_tests;
