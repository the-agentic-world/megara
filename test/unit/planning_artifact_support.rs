use std::fs;

use serde_json::{json, Value};
use tempfile::TempDir;

use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;

pub(crate) struct ArtifactHarness {
    pub(crate) directory: TempDir,
    pub(crate) service: PlanningService,
    pub(crate) session_id: String,
}

impl ArtifactHarness {
    pub(crate) fn new() -> Self {
        Self::with_initial_request("artifact contract request")
    }

    pub(crate) fn with_initial_request(initial_request: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::write(directory.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let mut service = PlanningService::open_project(directory.path()).unwrap();
        let started = service.handle_user_request(request(
            "planning.start",
            "cmd-artifact-start",
            None,
            None,
            json!({"request":initial_request, "title":"Artifact title"}),
        ));
        assert_ok(&started);
        let session_id = started["session_id"].as_str().unwrap().to_string();
        let evidence = service.handle_request(request(
            "planning.evidence.refresh",
            "cmd-artifact-evidence",
            Some(&session_id),
            Some(1),
            json!({
                "citations":[{
                    "temp_ref":"main",
                    "path":"src/main.rs",
                    "ranges":[],
                    "claim":"the project has a deterministic entry point"
                }]
            }),
        ));
        assert_ok(&evidence);
        let state = evidence["result"]["state"].clone();
        let work = state["required_model_action"].clone();
        let delta = service.handle_request(request(
            "planning.audit.apply",
            "cmd-artifact-delta",
            Some(&session_id),
            Some(evidence["revision"].as_u64().unwrap()),
            json!({
                "mode":"delta",
                "proposal": {
                    "schema":"megara.audit-proposal/v1",
                    "mode":"delta",
                    "work_item_id":work["work_item_id"],
                    "base_revision":work["base_revision"],
                    "base_domain_revision":work["base_domain_revision"],
                    "input_hash":work["input_hash"],
                    "readiness":"request_full_audit",
                    "next_question":null,
                    "entity_ops":entity_ops(),
                    "edge_ops":[{
                        "op":"add",
                        "kind":"has_acceptance_criterion",
                        "from":{"temp_ref":"requirement"},
                        "to":{"temp_ref":"criterion"},
                        "source_refs":[{"kind":"initial_request","id":"request"}]
                    }],
                    "blocker_ops":[],
                    "counterexample_review":null
                }
            }),
        ));
        assert_ok(&delta);
        let full_work = delta["result"]["state"]["required_model_action"].clone();
        let full = service.handle_request(request(
            "planning.audit.apply",
            "cmd-artifact-full",
            Some(&session_id),
            Some(delta["revision"].as_u64().unwrap()),
            json!({
                "mode":"full",
                "proposal": {
                    "schema":"megara.audit-proposal/v1",
                    "mode":"full",
                    "work_item_id":full_work["work_item_id"],
                    "base_revision":full_work["base_revision"],
                    "base_domain_revision":full_work["base_domain_revision"],
                    "input_hash":full_work["input_hash"],
                    "readiness":"ready",
                    "next_question":null,
                    "entity_ops":[],
                    "edge_ops":[],
                    "blocker_ops":[],
                    "counterexample_review":{
                        "performed":true,
                        "challenged_entity_ids":[],
                        "findings":[]
                    }
                }
            }),
        ));
        assert_ok(&full);
        Self {
            directory,
            service,
            session_id,
        }
    }

    pub(crate) fn generate_spec(&mut self, command_id: &str) -> Value {
        let request = self.spec_request(command_id, false);
        let response = self.service.handle_request(request);
        assert_ok(&response);
        assert_eq!(
            response["result"]["candidate"]["created_event_seq"],
            response["revision"]
        );
        assert_eq!(response["result"]["candidate"]["created_ordinal"], 0);
        response
    }

    pub(crate) fn spec_request(
        &mut self,
        command_id: &str,
        force: bool,
    ) -> crate::planning::protocol::LogicalRequest {
        let state = self.status_state();
        let work = state["required_model_action"].clone();
        let proposal = spec_proposal_for(&state, &work);
        request(
            "planning.spec.generate",
            command_id,
            Some(&self.session_id),
            Some(state["revision"].as_u64().unwrap()),
            json!({"proposal":proposal, "projection_policy":{"force":force}}),
        )
    }

    pub(crate) fn approve_spec(&mut self, command_id: &str) -> Value {
        let state = self.status_state();
        let candidate = state["spec"]["current_candidate"].clone();
        let response = self.service.handle_user_request(request(
            "planning.spec.approve",
            command_id,
            Some(&self.session_id),
            Some(state["revision"].as_u64().unwrap()),
            json!({
                "candidate_id":candidate["candidate_id"],
                "semantic_hash":candidate["semantic_hash"],
                "base_domain_revision":candidate["base_domain_revision"]
            }),
        ));
        assert_ok(&response);
        response
    }

    pub(crate) fn generate_plan(&mut self, command_id: &str) -> Value {
        let state = self.status_state();
        let proposal = plan_proposal_for(&state);
        let response = self.service.handle_request(request(
            "planning.plan.generate",
            command_id,
            Some(&self.session_id),
            Some(state["revision"].as_u64().unwrap()),
            json!({"proposal":proposal, "projection_policy":{"force":false}}),
        ));
        assert_ok(&response);
        assert_eq!(
            response["result"]["candidate"]["created_event_seq"],
            response["revision"]
        );
        assert_eq!(response["result"]["candidate"]["created_ordinal"], 0);
        response
    }

    pub(crate) fn approve_plan(&mut self, command_id: &str) -> Value {
        let state = self.status_state();
        let candidate = state["plan"]["current_candidate"].clone();
        let response = self.service.handle_user_request(request(
            "planning.plan.approve",
            command_id,
            Some(&self.session_id),
            Some(state["revision"].as_u64().unwrap()),
            json!({
                "candidate_id":candidate["candidate_id"],
                "semantic_hash":candidate["semantic_hash"],
                "base_plan_revision":candidate["base_plan_revision"]
            }),
        ));
        assert_ok(&response);
        response
    }

    pub(crate) fn complete(&mut self) -> Value {
        self.generate_spec("cmd-artifact-spec-generate");
        self.approve_spec("cmd-artifact-spec-approve");
        self.generate_plan("cmd-artifact-plan-generate");
        self.approve_plan("cmd-artifact-plan-approve")
    }

    pub(crate) fn status_state(&mut self) -> Value {
        let response = self.service.handle_request(request(
            "planning.status",
            "",
            Some(&self.session_id),
            None,
            Value::Null,
        ));
        assert_ok(&response);
        response["result"]["state"].clone()
    }
}

pub(crate) fn request(
    operation: &str,
    command_id: &str,
    session_id: Option<&str>,
    expected_revision: Option<u64>,
    params: Value,
) -> crate::planning::protocol::LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: format!("req-{command_id}"),
        operation: operation.to_string(),
        command_id: (!command_id.is_empty()).then(|| command_id.to_string()),
        session_id: session_id.map(str::to_string),
        expected_revision,
        params: (!params.is_null()).then_some(params),
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

pub(crate) fn spec_proposal_for(state: &Value, work: &Value) -> Value {
    let refs = |kind: &str| {
        state["entities"]["revisions"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|records| records.as_array()?.last())
            .filter(|record| record["kind"] == kind)
            .map(|record| json!({"id":record["entity_id"],"revision":record["revision"]}))
            .collect::<Vec<_>>()
    };
    let problem = refs("problem").into_iter().next().unwrap();
    json!({
        "schema":"megara.spec-proposal/v1",
        "work_item_id":work["work_item_id"],
        "base_revision":work["base_revision"],
        "base_domain_revision":work["base_domain_revision"],
        "audit_input_hash":state["full_audit"]["input_hash"],
        "title":"Artifact specification",
        "summary":"A traceable canonical specification",
        "problem_ref":problem,
        "outcome_refs":refs("outcome"),
        "decision_refs":refs("decision"),
        "decision_boundary_refs":refs("decision_boundary"),
        "requirement_refs":refs("requirement"),
        "acceptance_criterion_refs":refs("acceptance_criterion"),
        "constraint_refs":refs("constraint"),
        "non_goal_refs":refs("non_goal"),
        "assumption_refs":refs("assumption"),
        "risk_refs":refs("risk"),
        "advisories":[]
    })
}

pub(crate) fn plan_proposal_for(state: &Value) -> Value {
    let work = &state["required_model_action"];
    let spec_approval = &state["spec"]["approval"];
    let requirement = entity_ref(state, "requirement");
    let criterion = entity_ref(state, "acceptance_criterion");
    json!({
        "schema":"megara.plan-proposal/v1",
        "work_item_id":work["work_item_id"],
        "base_revision":work["base_revision"],
        "base_plan_revision":work["base_plan_revision"],
        "plan_input_hash":work["input_hash"],
        "spec":{
            "candidate_id":spec_approval["candidate_id"],
            "semantic_hash":spec_approval["semantic_hash"]
        },
        "baseline":{
            "commands":["cargo test"],
            "known_failure_policy":"stop on an unexpected failure"
        },
        "steps":[{
            "temp_ref":"step-main",
            "objective":"preserve the planning contract",
            "requirement_refs":[requirement],
            "depends_on":[],
            "change_surface":["src/planning"],
            "risks":[],
            "rollback_or_recovery":"restore the previous state"
        }],
        "verifications":[{
            "temp_ref":"verify-main",
            "acceptance_criterion_ref":criterion,
            "plan_step_refs":["step-main"],
            "method":"command",
            "procedure":"cargo test --all-targets",
            "expected_result":"all tests pass"
        }],
        "plan_risks":[]
    })
}

fn entity_ref(state: &Value, kind: &str) -> Value {
    state["entities"]["revisions"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|records| records.as_array()?.last())
        .find(|record| record["kind"] == kind)
        .map(|record| json!({"id":record["entity_id"],"revision":record["revision"]}))
        .unwrap()
}
