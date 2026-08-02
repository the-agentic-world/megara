use crate::planning::{domain::*, engine::*};
use crate::planning_support::*;

#[test]
fn spec_and_plan_approval_require_exact_binding_and_end_at_complete() {
    let (mut core, generated) = generated_spec_core();
    let approved = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash: "sha256:spec".to_string(),
            base_revision: generated.domain_revision,
        })
        .unwrap();
    let plan_work = approved.state.required_model_action.clone().unwrap();
    let plan = core
        .generate_plan(PlanCandidateCommand {
            session_id: generated.session_id.clone(),
            expected_revision: approved.state.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                base_plan_revision: approved.state.plan_revision,
                plan_input_hash: "sha256:plan-input".to_string(),
                semantic_hash: "sha256:plan".to_string(),
                spec_candidate_id: "cand_spec".to_string(),
                spec_semantic_hash: "sha256:spec".to_string(),
                content: serde_json::json!({"steps": []}),
                stale: false,
            },
        })
        .unwrap();
    assert_eq!(plan_work.kind, ModelActionKind::GeneratePlan);
    for (candidate_id, semantic_hash, base_revision) in [
        ("wrong_candidate", "sha256:plan", plan.state.plan_revision),
        ("cand_plan", "sha256:wrong", plan.state.plan_revision),
        ("cand_plan", "sha256:plan", plan.state.plan_revision + 1),
    ] {
        let before = core.state(&generated.session_id).unwrap().clone();
        let event_count = core.events().len();
        let error = core
            .approve_plan(ApprovalCommand {
                session_id: generated.session_id.clone(),
                expected_revision: before.revision,
                candidate_id: candidate_id.to_string(),
                semantic_hash: semantic_hash.to_string(),
                base_revision,
            })
            .unwrap_err();
        assert!(matches!(
            error,
            CoreError::CandidateNotFound(_) | CoreError::ApprovalBindingMismatch
        ));
        assert_eq!(core.events().len(), event_count);
        assert_eq!(core.state(&generated.session_id).unwrap(), &before);
    }
    let complete = core
        .approve_plan(ApprovalCommand {
            session_id: generated.session_id,
            expected_revision: plan.state.revision,
            candidate_id: "cand_plan".to_string(),
            semantic_hash: "sha256:plan".to_string(),
            base_revision: plan.state.plan_revision,
        })
        .unwrap();
    assert_eq!(complete.state.phase, LifecyclePhase::Complete);
    assert!(complete.state.assert_invariants().is_ok());
}

#[test]
fn each_spec_approval_binding_mismatch_is_atomic() {
    let cases = [
        ("wrong_candidate", "sha256:spec", 2),
        ("cand_spec", "sha256:wrong", 1),
        ("cand_spec", "sha256:spec", 4),
    ];
    for (candidate_id, semantic_hash, base_revision) in cases {
        let (mut core, before) = generated_spec_core();
        let event_count = core.events().len();
        let error = core
            .approve_spec(ApprovalCommand {
                session_id: before.session_id.clone(),
                expected_revision: before.revision,
                candidate_id: candidate_id.to_string(),
                semantic_hash: semantic_hash.to_string(),
                base_revision,
            })
            .unwrap_err();
        assert!(matches!(
            error,
            CoreError::CandidateNotFound(_) | CoreError::ApprovalBindingMismatch
        ));
        assert_eq!(core.events().len(), event_count);
        assert_eq!(core.state(&before.session_id).unwrap(), &before);
    }
}
