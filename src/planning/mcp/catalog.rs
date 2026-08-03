use std::borrow::Cow;

use rmcp::model::{JsonObject, MetaObject, Tool, ToolAnnotations};
use serde_json::{json, Map, Value};

#[derive(Clone, Copy)]
pub(crate) struct ToolSpec {
    pub(crate) name: &'static str,
    pub(crate) operation: &'static str,
    pub(crate) read_only: bool,
    pub(crate) prompt_required: bool,
    pub(crate) destructive: bool,
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "planning_start",
        operation: "planning.start",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_answer",
        operation: "planning.answer",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_status",
        operation: "planning.status",
        read_only: true,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_current",
        operation: "planning.current",
        read_only: true,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_list",
        operation: "planning.list",
        read_only: true,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_evidence_refresh",
        operation: "planning.evidence.refresh",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_audit_apply",
        operation: "planning.audit.apply",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_spec_generate",
        operation: "planning.spec.generate",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_spec_show",
        operation: "planning.spec.show",
        read_only: true,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_spec_approve",
        operation: "planning.spec.approve",
        read_only: false,
        prompt_required: true,
        destructive: false,
    },
    ToolSpec {
        name: "planning_spec_revise",
        operation: "planning.spec.revise",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_plan_generate",
        operation: "planning.plan.generate",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_plan_show",
        operation: "planning.plan.show",
        read_only: true,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_plan_approve",
        operation: "planning.plan.approve",
        read_only: false,
        prompt_required: true,
        destructive: false,
    },
    ToolSpec {
        name: "planning_plan_revise",
        operation: "planning.plan.revise",
        read_only: false,
        prompt_required: false,
        destructive: false,
    },
    ToolSpec {
        name: "planning_export",
        operation: "planning.export",
        read_only: false,
        prompt_required: false,
        destructive: true,
    },
    ToolSpec {
        name: "planning_purge",
        operation: "planning.purge",
        read_only: false,
        prompt_required: true,
        destructive: true,
    },
];

pub(crate) fn tool_spec(name: &str) -> Option<ToolSpec> {
    TOOLS.iter().copied().find(|tool| tool.name == name)
}

pub(crate) fn tool_catalog() -> Vec<Tool> {
    TOOLS.iter().copied().map(tool_value).collect()
}

pub(crate) fn tool_value(spec: ToolSpec) -> Tool {
    let annotations = ToolAnnotations::new()
        .read_only(spec.read_only)
        .destructive(spec.destructive)
        .idempotent(spec.read_only)
        .open_world(false);
    let mut tool = Tool::new(
        Cow::Borrowed(spec.name),
        if spec.read_only {
            format!("Run {} through PlanningService and use its returned state.", spec.operation)
        } else {
            format!(
                "Run {} through PlanningService; supply a stable command_id and reuse it for retries.",
                spec.operation
            )
        },
        tool_schema(spec.operation),
    )
    .with_annotations(annotations);
    if spec.prompt_required {
        let mut meta = Map::new();
        meta.insert("megara".to_string(), json!({"approval_mode": "prompt"}));
        tool = tool.with_meta(MetaObject::from(meta));
    }
    tool
}

fn tool_schema(operation: &str) -> JsonObject {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let string = || json!({"type": "string"});
    let integer = || json!({"type": "integer", "minimum": 0});
    let object = || json!({"type": "object"});
    let add = |properties: &mut Map<String, Value>, name: &str, value: Value| {
        properties.insert(name.to_string(), value);
    };
    match operation {
        "planning.start" => {
            add(&mut properties, "request", string());
            add(&mut properties, "title", string());
            add(&mut properties, "command_id", string());
            required.extend(["request", "command_id"]);
        }
        "planning.answer" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "question_id", string());
            add(&mut properties, "text", string());
            add(
                &mut properties,
                "selected_choice_ids",
                json!({"type":"array","items":string()}),
            );
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "question_id",
                "text",
                "command_id",
            ]);
        }
        "planning.status" | "planning.current" => add(&mut properties, "session_id", string()),
        "planning.list" => add(
            &mut properties,
            "phase",
            json!({"type":"string","enum":["interview","specification","planning","complete"]}),
        ),
        "planning.evidence.refresh" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(
                &mut properties,
                "citations",
                json!({"type":"array","items":object()}),
            );
            add(&mut properties, "command_id", string());
            required.extend(["session_id", "expected_revision", "citations", "command_id"]);
        }
        "planning.audit.apply" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(
                &mut properties,
                "mode",
                json!({"type":"string","enum":["delta","full"]}),
            );
            add(&mut properties, "proposal", object());
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "mode",
                "proposal",
                "command_id",
            ]);
        }
        "planning.spec.generate" | "planning.plan.generate" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "proposal", object());
            add(&mut properties, "projection_policy", object());
            add(&mut properties, "command_id", string());
            required.extend(["session_id", "expected_revision", "proposal", "command_id"]);
        }
        "planning.spec.show" | "planning.plan.show" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "candidate_id", string());
            add(
                &mut properties,
                "format",
                json!({"type":"string","enum":["markdown","json"]}),
            );
        }
        "planning.spec.approve" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "candidate_id", string());
            add(&mut properties, "semantic_hash", string());
            add(&mut properties, "base_domain_revision", integer());
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "candidate_id",
                "semantic_hash",
                "base_domain_revision",
                "command_id",
            ]);
        }
        "planning.plan.approve" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "candidate_id", string());
            add(&mut properties, "semantic_hash", string());
            add(&mut properties, "base_plan_revision", integer());
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "candidate_id",
                "semantic_hash",
                "base_plan_revision",
                "command_id",
            ]);
        }
        "planning.spec.revise" | "planning.plan.revise" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "candidate_id", string());
            add(&mut properties, "text", string());
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "candidate_id",
                "text",
                "command_id",
            ]);
        }
        "planning.export" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "out", string());
            add(
                &mut properties,
                "format",
                json!({"type":"string","enum":["bundle","state-json","events-jsonl"]}),
            );
            add(
                &mut properties,
                "include_transcript",
                json!({"type":"boolean"}),
            );
            add(&mut properties, "force", json!({"type":"boolean"}));
            add(&mut properties, "command_id", string());
            required.extend(["out", "format", "command_id"]);
        }
        "planning.purge" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(&mut properties, "confirm", string());
            add(&mut properties, "command_id", string());
            required.extend(["session_id", "expected_revision", "confirm", "command_id"]);
        }
        _ => {}
    }
    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
        "additionalProperties": false
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}
