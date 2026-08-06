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
                json!({"type":"array","items":{"$ref":"#/$defs/citation"}}),
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
            add(
                &mut properties,
                "proposal",
                json!({"$ref":"#/$defs/audit_proposal"}),
            );
            add(&mut properties, "command_id", string());
            required.extend([
                "session_id",
                "expected_revision",
                "mode",
                "proposal",
                "command_id",
            ]);
        }
        "planning.spec.generate" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(
                &mut properties,
                "proposal",
                json!({"$ref":"#/$defs/spec_proposal"}),
            );
            add(
                &mut properties,
                "projection_policy",
                json!({"$ref":"#/$defs/projection_policy"}),
            );
            add(&mut properties, "command_id", string());
            required.extend(["session_id", "expected_revision", "proposal", "command_id"]);
        }
        "planning.plan.generate" => {
            add(&mut properties, "session_id", string());
            add(&mut properties, "expected_revision", integer());
            add(
                &mut properties,
                "proposal",
                json!({"$ref":"#/$defs/plan_proposal"}),
            );
            add(
                &mut properties,
                "projection_policy",
                json!({"$ref":"#/$defs/projection_policy"}),
            );
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
    let mut schema = json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
        "additionalProperties": false,
    });
    if let Some(definitions) = proposal_definitions(operation) {
        schema
            .as_object_mut()
            .expect("tool schema is an object")
            .insert("$defs".to_string(), Value::Object(definitions));
    }
    schema.as_object_mut().cloned().unwrap_or_default()
}

fn proposal_definitions(operation: &str) -> Option<Map<String, Value>> {
    let required = match operation {
        "planning.evidence.refresh" => &["citation", "citation_range"][..],
        "planning.audit.apply" => &[
            "source_ref",
            "choice",
            "recommendation",
            "question",
            "counterexample_review",
            "entity_op",
            "edge_op",
            "blocker_op",
            "audit_proposal",
        ][..],
        "planning.spec.generate" => &[
            "source_ref",
            "entity_ref",
            "projection_policy",
            "spec_proposal",
        ][..],
        "planning.plan.generate" => &["entity_ref", "projection_policy", "plan_proposal"][..],
        _ => return None,
    };
    let mut definitions = Map::new();
    definitions.insert("source_ref".to_string(), source_ref_schema());
    definitions.insert("entity_ref".to_string(), entity_ref_schema());
    definitions.insert(
        "citation_range".to_string(),
        strict_object(
            json!({
                "start_line": {"type": "integer", "minimum": 1},
                "end_line": {"type": "integer", "minimum": 1},
            }),
            &["start_line", "end_line"],
        ),
    );
    definitions.insert(
        "citation".to_string(),
        strict_object(
            json!({
                "temp_ref": {"type": "string"},
                "path": {"type": "string"},
                "ranges": {"type": "array", "items": {"$ref": "#/$defs/citation_range"}},
                "claim": {"type": "string"},
            }),
            &["temp_ref", "path", "ranges", "claim"],
        ),
    );
    definitions.insert("choice".to_string(), strict_object(json!({
        "id": {"type": "string"}, "label": {"type": "string"}, "direction": {"type": "string"},
        "benefits": {"type": "array", "items": {"type": "string"}},
        "tradeoffs": {"type": "array", "items": {"type": "string"}},
    }), &["id", "label", "direction", "benefits", "tradeoffs"]));
    definitions.insert(
        "recommendation".to_string(),
        strict_object(
            json!({
                "choice_id": {"type": "string"}, "reason": {"type": "string"},
                "source_refs": {"type": "array", "items": {"$ref": "#/$defs/source_ref"}},
            }),
            &["choice_id", "reason", "source_refs"],
        ),
    );
    definitions.insert("question".to_string(), question_schema());
    definitions.insert(
        "counterexample_review".to_string(),
        counterexample_review_schema(),
    );
    definitions.insert("entity_op".to_string(), entity_op_schema());
    definitions.insert("edge_op".to_string(), edge_op_schema());
    definitions.insert("blocker_op".to_string(), blocker_op_schema());
    definitions.insert("audit_proposal".to_string(), audit_proposal_schema());
    definitions.insert(
        "projection_policy".to_string(),
        strict_object(
            json!({
                "force": {"type": "boolean"},
            }),
            &[],
        ),
    );
    definitions.insert("spec_proposal".to_string(), spec_proposal_schema());
    definitions.insert("plan_proposal".to_string(), plan_proposal_schema());
    definitions.retain(|name, _| required.contains(&name.as_str()));
    Some(definitions)
}

fn strict_object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object", "properties": properties,
        "required": required, "additionalProperties": false,
    })
}

fn source_ref_schema() -> Value {
    json!({"oneOf": [
        strict_object(json!({"kind":{"const":"initial_request"}, "id":{"type":"string"}}), &["kind", "id"]),
        strict_object(json!({"kind":{"const":"answer"}, "id":{"type":"string"}}), &["kind", "id"]),
        strict_object(json!({"kind":{"const":"evidence"}, "id":{"type":"string"}}), &["kind", "id"]),
        strict_object(json!({"kind":{"const":"entity"}, "id":{"type":"string"}, "revision":{"type":"integer","minimum":0}}), &["kind", "id", "revision"]),
        strict_object(json!({"kind":{"const":"approved_spec"}, "id":{"type":"string"}, "semantic_hash":{"type":"string"}}), &["kind", "id", "semantic_hash"]),
    ]})
}

fn entity_ref_schema() -> Value {
    strict_object(
        json!({
            "id": {"type": "string"}, "revision": {"type": "integer", "minimum": 0},
        }),
        &["id", "revision"],
    )
}

fn question_schema() -> Value {
    let choice_answer = strict_object(
        json!({
            "mode": {"const": "choice"},
            "choices": {"type":"array", "items":{"$ref":"#/$defs/choice"}},
            "recommendation": {"anyOf":[{"$ref":"#/$defs/recommendation"},{"type":"null"}]},
            "freeform_hint": {"type":"string"},
        }),
        &["mode", "choices", "recommendation", "freeform_hint"],
    );
    let freeform_answer = strict_object(
        json!({
            "mode": {"const": "freeform"}, "freeform_hint": {"type":"string"},
        }),
        &["mode", "freeform_hint"],
    );
    strict_object(
        json!({
            "context": {"type":"string"}, "question": {"type":"string"}, "why_it_matters": {"type":"string"},
            "technical_terms": {"type":"array", "items": strict_object(json!({"term":{"type":"string"},"plain_explanation":{"type":"string"}}), &["term", "plain_explanation"])},
            "source_refs": {"type":"array", "items":{"$ref":"#/$defs/source_ref"}},
            "answer": {"oneOf":[choice_answer, freeform_answer]},
        }),
        &[
            "context",
            "question",
            "why_it_matters",
            "technical_terms",
            "source_refs",
            "answer",
        ],
    )
}

fn counterexample_review_schema() -> Value {
    strict_object(
        json!({
            "performed": {"type":"boolean"},
            "challenged_entity_ids": {"type":"array", "items":{"type":"string"}},
            "findings": {"type":"array", "items": strict_object(json!({
                "statement":{"type":"string"}, "result":{"enum":["resolved","blocking","advisory"]},
                "source_refs":{"type":"array", "items":{"$ref":"#/$defs/source_ref"}},
            }), &["statement", "result", "source_refs"])},
        }),
        &["performed", "challenged_entity_ids", "findings"],
    )
}

fn entity_op_schema() -> Value {
    let mut creates = Vec::new();
    for (kind, body) in [
        ("problem", json!({"statement":{"type":"string"}})),
        (
            "outcome",
            json!({"statement":{"type":"string"},"observable_result":{"type":"string"}}),
        ),
        (
            "fact",
            json!({"statement":{"type":"string"},"evidence_refs":{"type":"array","items":{"type":"string"}}}),
        ),
        (
            "decision",
            json!({"statement":{"type":"string"},"selected_option":{"type":"string"}}),
        ),
        (
            "decision_boundary",
            json!({"autonomous_scope":{"type":"array","items":{"type":"string"}},"requires_user_approval":{"type":"array","items":{"type":"string"}}}),
        ),
        (
            "requirement",
            json!({"statement":{"type":"string"},"priority":{"enum":["must","should","could"]}}),
        ),
        (
            "acceptance_criterion",
            json!({"statement":{"type":"string"}}),
        ),
        ("constraint", json!({"statement":{"type":"string"}})),
        ("non_goal", json!({"statement":{"type":"string"}})),
        (
            "assumption",
            json!({"statement":{"type":"string"},"validation_status":{"enum":["unverified","confirmed","rejected"]}}),
        ),
        (
            "risk",
            json!({"statement":{"type":"string"},"impact":{"enum":["low","medium","high","critical"]},"mitigation":{"type":"string"}}),
        ),
    ] {
        let required = body
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let required_refs = required.iter().map(String::as_str).collect::<Vec<_>>();
        creates.push(strict_object(
            json!({
                "op":{"const":"create"}, "temp_ref":{"type":"string"}, "kind":{"const":kind},
                "body": strict_object(body, &required_refs),
                "source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}},
            }),
            &["op", "temp_ref", "kind", "body", "source_refs"],
        ));
    }
    creates.push(strict_object(json!({"op":{"const":"revise"},"entity_id":{"type":"string"},"base_entity_revision":{"type":"integer","minimum":0},"body":audit_entity_body_schema(),"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["op","entity_id","base_entity_revision","body","source_refs"]));
    creates.push(strict_object(json!({"op":{"const":"reject"},"entity_id":{"type":"string"},"base_entity_revision":{"type":"integer","minimum":0},"reason":{"type":"string"},"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["op","entity_id","base_entity_revision","reason","source_refs"]));
    json!({"oneOf": creates})
}

fn audit_entity_body_schema() -> Value {
    json!({"oneOf": [
        strict_object(json!({"statement":{"type":"string"}}), &["statement"]),
        strict_object(json!({"statement":{"type":"string"},"observable_result":{"type":"string"}}), &["statement","observable_result"]),
        strict_object(json!({"statement":{"type":"string"},"evidence_refs":{"type":"array","items":{"type":"string"}}}), &["statement","evidence_refs"]),
        strict_object(json!({"statement":{"type":"string"},"selected_option":{"type":"string"}}), &["statement","selected_option"]),
        strict_object(json!({"autonomous_scope":{"type":"array","items":{"type":"string"}},"requires_user_approval":{"type":"array","items":{"type":"string"}}}), &["autonomous_scope","requires_user_approval"]),
        strict_object(json!({"statement":{"type":"string"},"priority":{"enum":["must","should","could"]}}), &["statement","priority"]),
        strict_object(json!({"statement":{"type":"string"},"validation_status":{"enum":["unverified","confirmed","rejected"]}}), &["statement","validation_status"]),
        strict_object(json!({"statement":{"type":"string"},"impact":{"enum":["low","medium","high","critical"]},"mitigation":{"type":"string"}}), &["statement","impact","mitigation"]),
    ]})
}

fn audit_endpoint_schema() -> Value {
    json!({"oneOf": [
        strict_object(json!({"temp_ref":{"type":"string"}}), &["temp_ref"]),
        strict_object(json!({"entity_id":{"type":"string"},"revision":{"type":"integer","minimum":0}}), &["entity_id", "revision"]),
        {"$ref":"#/$defs/source_ref"},
    ]})
}

fn edge_op_schema() -> Value {
    json!({"oneOf": [
        strict_object(json!({"op":{"const":"add"},"kind":{"enum":["has_acceptance_criterion","implements","verifies","executed_by","depends_on","derived_from","supersedes","conflicts_with"]},"from":audit_endpoint_schema(),"to":audit_endpoint_schema(),"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["op","kind","from","to","source_refs"]),
        strict_object(json!({"op":{"const":"retire"},"edge_id":{"type":"string"},"base_edge_revision":{"type":"integer","minimum":0},"reason":{"type":"string"}}), &["op","edge_id","base_edge_revision","reason"]),
    ]})
}

fn blocker_op_schema() -> Value {
    json!({"oneOf": [
        strict_object(json!({"op":{"const":"create"},"temp_ref":{"type":"string"},"kind":{"enum":["missing_problem","missing_outcome","missing_requirement","missing_non_goal","missing_decision_boundary","missing_acceptance_criterion","open_decision","contradiction","evidence_required","invalid_source","model_output_invalid","manual_review_required"]},"severity":{"enum":["blocking","advisory"]},"statement":{"type":"string"},"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["op","temp_ref","kind","severity","statement","source_refs"]),
        strict_object(json!({"op":{"const":"resolve"},"blocker_id":{"type":"string"},"base_blocker_revision":{"type":"integer","minimum":0},"resolution":{"type":"string"},"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["op","blocker_id","base_blocker_revision","resolution","source_refs"]),
    ]})
}

fn audit_proposal_schema() -> Value {
    strict_object(
        json!({
            "schema":{"const":"megara.audit-proposal/v1"}, "mode":{"enum":["delta","full"]},
            "work_item_id":{"type":"string"}, "base_revision":{"type":"integer","minimum":0},
            "base_domain_revision":{"type":"integer","minimum":0}, "input_hash":{"type":"string"},
            "readiness":{"enum":["continue","request_full_audit","ready"]},
            "next_question":{"anyOf":[{"$ref":"#/$defs/question"},{"type":"null"}]},
            "entity_ops":{"type":"array","items":{"$ref":"#/$defs/entity_op"}},
            "edge_ops":{"type":"array","items":{"$ref":"#/$defs/edge_op"}},
            "blocker_ops":{"type":"array","items":{"$ref":"#/$defs/blocker_op"}},
            "counterexample_review":{"anyOf":[{"$ref":"#/$defs/counterexample_review"},{"type":"null"}]},
        }),
        &[
            "schema",
            "mode",
            "work_item_id",
            "base_revision",
            "base_domain_revision",
            "input_hash",
            "readiness",
            "next_question",
            "entity_ops",
            "edge_ops",
            "blocker_ops",
            "counterexample_review",
        ],
    )
}

fn spec_proposal_schema() -> Value {
    let refs = json!({"type":"array","items":{"$ref":"#/$defs/entity_ref"}});
    strict_object(
        json!({
            "schema":{"const":"megara.spec-proposal/v1"},"work_item_id":{"type":"string"},"base_revision":{"type":"integer","minimum":0},"base_domain_revision":{"type":"integer","minimum":0},"audit_input_hash":{"type":"string"},"title":{"type":"string"},"summary":{"type":"string"},"problem_ref":{"$ref":"#/$defs/entity_ref"},
            "outcome_refs":refs,"decision_refs":refs,"decision_boundary_refs":refs,"requirement_refs":refs,"acceptance_criterion_refs":refs,"constraint_refs":refs,"non_goal_refs":refs,"assumption_refs":refs,"risk_refs":refs,
            "advisories":{"type":"array","items":strict_object(json!({"statement":{"type":"string"},"source_refs":{"type":"array","items":{"$ref":"#/$defs/source_ref"}}}), &["statement","source_refs"])},
        }),
        &[
            "schema",
            "work_item_id",
            "base_revision",
            "base_domain_revision",
            "audit_input_hash",
            "title",
            "summary",
            "problem_ref",
            "outcome_refs",
            "decision_refs",
            "decision_boundary_refs",
            "requirement_refs",
            "acceptance_criterion_refs",
            "constraint_refs",
            "non_goal_refs",
            "assumption_refs",
            "risk_refs",
            "advisories",
        ],
    )
}

fn plan_proposal_schema() -> Value {
    strict_object(
        json!({
            "schema":{"const":"megara.plan-proposal/v1"},"work_item_id":{"type":"string"},"base_revision":{"type":"integer","minimum":0},"base_plan_revision":{"type":"integer","minimum":0},"plan_input_hash":{"type":"string"},
            "spec":strict_object(json!({"candidate_id":{"type":"string"},"semantic_hash":{"type":"string"}}), &["candidate_id","semantic_hash"]),
            "baseline":strict_object(json!({"commands":{"type":"array","items":{"type":"string"}},"known_failure_policy":{"type":"string"}}), &["commands","known_failure_policy"]),
            "steps":{"type":"array","items":strict_object(json!({"temp_ref":{"type":"string"},"objective":{"type":"string"},"requirement_refs":{"type":"array","items":{"$ref":"#/$defs/entity_ref"}},"depends_on":{"type":"array","items":{"type":"string"}},"change_surface":{"type":"array","items":{"type":"string"}},"risks":{"type":"array","items":{"type":"string"}},"rollback_or_recovery":{"type":"string"}}), &["temp_ref","objective","requirement_refs","depends_on","change_surface","risks","rollback_or_recovery"])},
            "verifications":{"type":"array","items":strict_object(json!({"temp_ref":{"type":"string"},"acceptance_criterion_ref":{"$ref":"#/$defs/entity_ref"},"plan_step_refs":{"type":"array","items":{"type":"string"}},"method":{"enum":["command","assertion","metric","manual"]},"procedure":{"type":"string"},"expected_result":{"type":"string"}}), &["temp_ref","acceptance_criterion_ref","plan_step_refs","method","procedure","expected_result"])},
            "plan_risks":{"type":"array","items":strict_object(json!({"statement":{"type":"string"},"mitigation":{"type":"string"}}), &["statement","mitigation"])},
        }),
        &[
            "schema",
            "work_item_id",
            "base_revision",
            "base_plan_revision",
            "plan_input_hash",
            "spec",
            "baseline",
            "steps",
            "verifications",
            "plan_risks",
        ],
    )
}
