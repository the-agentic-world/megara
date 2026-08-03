use std::path::Path;

use serde_json::{json, Value};

use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;

pub(crate) fn prepare_complete(project: &Path, session: &str) {
    let (mut service, full) = prepare_at_specification(project, session);
    let state = full["result"]["state"].clone();
    let work = state["required_model_action"].clone();
    let spec = spec_proposal(&state, &work);
    let generated = service.handle_request(request(
        "planning.spec.generate",
        "cmd-cli-artifact-spec",
        session,
        full["revision"].as_u64().unwrap(),
        json!({"proposal":spec,"projection_policy":{"force":false}}),
    ));
    assert_ok(&generated);
    let state = generated["result"]["state"].clone();
    let candidate = state["spec"]["current_candidate"].clone();
    let approved = service.handle_user_request(request(
        "planning.spec.approve",
        "cmd-cli-artifact-spec-approve",
        session,
        generated["revision"].as_u64().unwrap(),
        json!({"candidate_id":candidate["candidate_id"],"semantic_hash":candidate["semantic_hash"],"base_domain_revision":candidate["base_domain_revision"]}),
    ));
    assert_ok(&approved);
    let state = approved["result"]["state"].clone();
    let work = state["required_model_action"].clone();
    let spec_approval = state["spec"]["approval"].clone();
    let plan = plan_proposal(&state, &work, &spec_approval);
    let generated = service.handle_request(request(
        "planning.plan.generate",
        "cmd-cli-artifact-plan",
        session,
        approved["revision"].as_u64().unwrap(),
        json!({"proposal":plan,"projection_policy":{"force":false}}),
    ));
    assert_ok(&generated);
    let state = generated["result"]["state"].clone();
    let candidate = state["plan"]["current_candidate"].clone();
    let complete = service.handle_user_request(request(
        "planning.plan.approve",
        "cmd-cli-artifact-plan-approve",
        session,
        generated["revision"].as_u64().unwrap(),
        json!({"candidate_id":candidate["candidate_id"],"semantic_hash":candidate["semantic_hash"],"base_plan_revision":candidate["base_plan_revision"]}),
    ));
    assert_ok(&complete);
}

pub(crate) fn prepare_at_specification(project: &Path, session: &str) -> (PlanningService, Value) {
    let mut service = PlanningService::open_project(project).unwrap();
    let evidence = service.handle_request(request(
        "planning.evidence.refresh",
        "cmd-cli-artifact-evidence",
        session,
        1,
        json!({"citations":[{"temp_ref":"main","path":"src/main.rs","ranges":[],"claim":"entry point"}]}),
    ));
    assert_ok(&evidence);
    let state = evidence["result"]["state"].clone();
    let work = state["required_model_action"].clone();
    let delta = service.handle_request(request(
        "planning.audit.apply",
        "cmd-cli-artifact-delta",
        session,
        evidence["revision"].as_u64().unwrap(),
        json!({
            "mode":"delta",
            "proposal":{
                "schema":"megara.audit-proposal/v1","mode":"delta",
                "work_item_id":work["work_item_id"],"base_revision":work["base_revision"],
                "base_domain_revision":work["base_domain_revision"],"input_hash":work["input_hash"],
                "readiness":"request_full_audit","next_question":null,"entity_ops":entity_ops(),
                "edge_ops":[{"op":"add","kind":"has_acceptance_criterion","from":{"temp_ref":"requirement"},"to":{"temp_ref":"criterion"},"source_refs":[{"kind":"initial_request","id":"request"}]}],
                "blocker_ops":[],"counterexample_review":null
            }
        }),
    ));
    assert_ok(&delta);
    let full_work = delta["result"]["state"]["required_model_action"].clone();
    let full = service.handle_request(request(
        "planning.audit.apply",
        "cmd-cli-artifact-full",
        session,
        delta["revision"].as_u64().unwrap(),
        json!({"mode":"full","proposal":{
            "schema":"megara.audit-proposal/v1","mode":"full",
            "work_item_id":full_work["work_item_id"],"base_revision":full_work["base_revision"],
            "base_domain_revision":full_work["base_domain_revision"],"input_hash":full_work["input_hash"],
            "readiness":"ready","next_question":null,"entity_ops":[],"edge_ops":[],"blocker_ops":[],
            "counterexample_review":{"performed":true,"challenged_entity_ids":[],"findings":[]}
        }}),
    ));
    assert_ok(&full);
    (service, full)
}

pub(crate) fn request(
    operation: &str,
    command_id: &str,
    session: &str,
    expected_revision: u64,
    params: Value,
) -> LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: format!("req-{command_id}"),
        operation: operation.to_string(),
        command_id: Some(command_id.to_string()),
        session_id: Some(session.to_string()),
        expected_revision: Some(expected_revision),
        params: Some(params),
    }
}

pub(crate) fn assert_ok(value: &Value) {
    assert_eq!(value["ok"], true, "response={value}");
}

fn entity_ops() -> Value {
    let source = json!([{"kind":"initial_request","id":"request"}]);
    json!([
        {"op":"create","temp_ref":"problem","kind":"problem","body":{"statement":"the problem"},"source_refs":source},
        {"op":"create","temp_ref":"outcome","kind":"outcome","body":{"statement":"the outcome","observable_result":"observable"},"source_refs":source},
        {"op":"create","temp_ref":"requirement","kind":"requirement","body":{"statement":"the requirement","priority":"must"},"source_refs":source},
        {"op":"create","temp_ref":"non_goal","kind":"non_goal","body":{"statement":"outside scope"},"source_refs":source},
        {"op":"create","temp_ref":"boundary","kind":"decision_boundary","body":{"autonomous_scope":["validation"],"requires_user_approval":["approval"]},"source_refs":source},
        {"op":"create","temp_ref":"criterion","kind":"acceptance_criterion","body":{"statement":"the criterion"},"source_refs":source}
    ])
}

fn refs(state: &Value, kind: &str) -> Vec<Value> {
    state["entities"]["revisions"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|records| records.as_array()?.last())
        .filter(|record| record["kind"] == kind)
        .map(|record| json!({"id":record["entity_id"],"revision":record["revision"]}))
        .collect()
}

fn one_ref(state: &Value, kind: &str) -> Value {
    refs(state, kind).into_iter().next().unwrap()
}

pub(crate) fn spec_proposal(state: &Value, work: &Value) -> Value {
    json!({
        "schema":"megara.spec-proposal/v1","work_item_id":work["work_item_id"],"base_revision":work["base_revision"],
        "base_domain_revision":work["base_domain_revision"],"audit_input_hash":state["full_audit"]["input_hash"],
        "title":"A traceable canonical specification","summary":"A traceable canonical specification",
        "problem_ref":one_ref(state,"problem"),"outcome_refs":refs(state,"outcome"),"decision_refs":refs(state,"decision"),
        "decision_boundary_refs":refs(state,"decision_boundary"),"requirement_refs":refs(state,"requirement"),
        "acceptance_criterion_refs":refs(state,"acceptance_criterion"),"constraint_refs":refs(state,"constraint"),
        "non_goal_refs":refs(state,"non_goal"),"assumption_refs":refs(state,"assumption"),"risk_refs":refs(state,"risk"),"advisories":[]
    })
}

pub(crate) fn plan_proposal(state: &Value, work: &Value, approval: &Value) -> Value {
    json!({
        "schema":"megara.plan-proposal/v1","work_item_id":work["work_item_id"],"base_revision":work["base_revision"],
        "base_plan_revision":work["base_plan_revision"],"plan_input_hash":work["input_hash"],
        "spec":{"candidate_id":approval["candidate_id"],"semantic_hash":approval["semantic_hash"]},
        "baseline":{"commands":["cargo test"],"known_failure_policy":"stop"},
        "steps":[{"temp_ref":"step-main","objective":"preserve the contract","requirement_refs":[one_ref(state,"requirement")],"depends_on":[],"change_surface":["src"],"risks":[],"rollback_or_recovery":"restore"}],
        "verifications":[{"temp_ref":"verify-main","acceptance_criterion_ref":one_ref(state,"acceptance_criterion"),"plan_step_refs":["step-main"],"method":"command","procedure":"cargo test","expected_result":"pass"}],
        "plan_risks":[]
    })
}
