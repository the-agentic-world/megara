#![allow(dead_code)]

#[path = "../../src/agents.rs"]
mod agents;
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
#[path = "../../src/planning.rs"]
mod planning;
#[path = "../../src/targets.rs"]
mod targets;
#[path = "../../src/team.rs"]
mod team;
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

pub(crate) use hook::codex_version::{is_outdated, parse_numeric_version};
pub(crate) use hook::fsutil::append_jsonl;
pub(crate) use hook::mutation::mutating_command;
pub(crate) use hook::parser::{
    approval_gate_from_text, parse_blocks, text_before_first_workflow_block,
};
pub(crate) use hook::runtime_input::{
    assistant_message_from_payload, effective_prompt_from_payload, effective_prompt_text,
    runtime_context, RuntimeSurface,
};
pub(crate) use hook::state_paths::workflow_paths;
pub(crate) use installer::{PlannedFile, MANAGED_MARKER};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::{fs, path::Path};
pub(crate) use targets::codex::role_profile;
pub(crate) use team::split::codex_exec_args;
pub(crate) use ultragoal::*;
pub(crate) use writer::*;

#[path = "docs.rs"]
mod docs_tests;
#[path = "hook.rs"]
mod hook_tests;
#[path = "pi.rs"]
mod pi_tests;
#[path = "planning_approval.rs"]
mod planning_approval;
#[path = "planning_artifact_evidence.rs"]
mod planning_artifact_evidence;
#[path = "planning_artifact_support.rs"]
mod planning_artifact_support;
#[path = "planning_artifacts.rs"]
mod planning_artifacts;
#[path = "planning_audit_combinations.rs"]
mod planning_audit_combinations;
#[path = "planning_audit_readiness.rs"]
mod planning_audit_readiness;
#[path = "planning_audit_support.rs"]
mod planning_audit_support;
#[path = "planning_domain.rs"]
mod planning_domain;
#[path = "planning_edge_wire.rs"]
mod planning_edge_wire;
#[path = "planning_engine.rs"]
mod planning_engine;
#[path = "planning_evidence.rs"]
mod planning_evidence;
#[path = "planning_evidence_security.rs"]
mod planning_evidence_security;
#[path = "planning_export.rs"]
mod planning_export;
#[path = "planning_invalidation.rs"]
mod planning_invalidation;
#[path = "planning_invalidation_derived.rs"]
mod planning_invalidation_derived;
#[path = "planning_projection.rs"]
mod planning_projection;
#[path = "planning_protocol.rs"]
mod planning_protocol;
#[path = "planning_protocol_golden.rs"]
mod planning_protocol_golden;
#[path = "planning_question.rs"]
mod planning_question;
#[path = "planning_question_contract.rs"]
mod planning_question_contract;
#[path = "planning_question_transition.rs"]
mod planning_question_transition;
#[path = "planning_service.rs"]
mod planning_service;
#[path = "planning_service_health.rs"]
mod planning_service_health;
#[path = "planning_service_support.rs"]
mod planning_service_support;
#[path = "planning_service_wire.rs"]
mod planning_service_wire;
#[path = "planning_service_wire_invalid.rs"]
mod planning_service_wire_invalid;
#[path = "planning_service_wire_support.rs"]
mod planning_service_wire_support;
#[path = "planning_store.rs"]
mod planning_store;
#[path = "planning_store_concurrency.rs"]
mod planning_store_concurrency;
#[path = "planning_store_evidence_scan.rs"]
mod planning_store_evidence_scan;
#[path = "planning_store_hash.rs"]
mod planning_store_hash;
#[path = "planning_store_purge.rs"]
mod planning_store_purge;
#[path = "planning_store_schema.rs"]
mod planning_store_schema;
#[path = "planning_store_support.rs"]
mod planning_store_support;
#[path = "planning_support.rs"]
mod planning_support;
#[path = "planning_transitions.rs"]
mod planning_transitions;
#[path = "planning_work_items.rs"]
mod planning_work_items;
#[path = "team.rs"]
mod team_tests;
#[path = "tui.rs"]
mod tui_tests;
#[path = "ultragoal.rs"]
mod ultragoal_tests;
#[path = "update.rs"]
mod update_tests;
#[path = "writer.rs"]
mod writer_tests;
