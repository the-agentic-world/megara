use crate::planning::{domain::*, engine::*};
use crate::planning_support::*;

#[test]
fn spec_and_plan_approval_require_exact_binding_and_end_at_complete() {
    let (mut core, generated) = generated_spec_core();
    let spec_hash = generated
        .spec
        .current_candidate
        .as_ref()
        .unwrap()
        .semantic_hash
        .clone();
    let approved = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash: spec_hash.clone(),
            base_revision: generated.domain_revision,
        })
        .unwrap();
    let plan_work = approved.state.required_model_action.clone().unwrap();
    let requirement = approved.state.entities.current_requirements()[0];
    let criterion = approved.state.entities.current_acceptance_criteria()[0];
    let plan_content = serde_json::json!({
        "baseline": {
            "commands": ["cargo test"],
            "known_failure_policy": "stop"
        },
        "steps": [{
            "temp_ref": "step-1",
            "objective": "검증 가능한 계획을 만든다.",
            "requirement_refs": [{
                "id": requirement.entity_id,
                "revision": requirement.revision
            }],
            "depends_on": [],
            "change_surface": ["src"],
            "risks": [],
            "rollback_or_recovery": "이전 상태로 복구한다."
        }],
        "verifications": [{
            "temp_ref": "verify-1",
            "acceptance_criterion_ref": {
                "id": criterion.entity_id,
                "revision": criterion.revision
            },
            "plan_step_refs": ["step-1"],
            "method": "command",
            "procedure": "cargo test",
            "expected_result": "통과한다."
        }],
        "plan_risks": []
    });
    let plan_hash = crate::planning::canonical::canonical_hash(&plan_content);
    let plan = core
        .generate_plan(PlanCandidateCommand {
            session_id: generated.session_id.clone(),
            expected_revision: approved.state.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                created_event_seq: approved.state.revision + 1,
                created_ordinal: 0,
                base_plan_revision: approved.state.plan_revision,
                plan_input_hash: plan_work.input_hash.clone(),
                semantic_hash: plan_hash.clone(),
                spec_candidate_id: "cand_spec".to_string(),
                spec_semantic_hash: spec_hash,
                content: plan_content,
                stale: false,
            },
        })
        .unwrap();
    assert_eq!(plan_work.kind, ModelActionKind::GeneratePlan);
    for mismatch in ["candidate", "semantic_hash", "base_revision"] {
        let before = core.state(&generated.session_id).unwrap().clone();
        let candidate = before.plan.current_candidate.as_ref().unwrap();
        let (candidate_id, semantic_hash, base_revision) = match mismatch {
            "candidate" => (
                "wrong_candidate".to_string(),
                candidate.semantic_hash.clone(),
                candidate.base_plan_revision,
            ),
            "semantic_hash" => (
                candidate.candidate_id.clone(),
                format!("{}-tampered", candidate.semantic_hash),
                candidate.base_plan_revision,
            ),
            _ => (
                candidate.candidate_id.clone(),
                candidate.semantic_hash.clone(),
                candidate.base_plan_revision + 1,
            ),
        };
        let event_count = core.events().len();
        let error = core
            .approve_plan(ApprovalCommand {
                session_id: generated.session_id.clone(),
                expected_revision: before.revision,
                candidate_id,
                semantic_hash,
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
            semantic_hash: plan_hash,
            base_revision: plan.state.plan_revision,
        })
        .unwrap();
    assert_eq!(complete.state.phase, LifecyclePhase::Complete);
    assert!(complete.state.assert_invariants().is_ok());
}

#[test]
fn each_spec_approval_binding_mismatch_is_atomic() {
    for mismatch in ["candidate", "semantic_hash", "base_revision"] {
        let (mut core, before) = generated_spec_core();
        let candidate = before.spec.current_candidate.as_ref().unwrap();
        let (candidate_id, semantic_hash, base_revision) = match mismatch {
            "candidate" => (
                "wrong_candidate".to_string(),
                candidate.semantic_hash.clone(),
                candidate.base_domain_revision,
            ),
            "semantic_hash" => (
                candidate.candidate_id.clone(),
                format!("{}-tampered", candidate.semantic_hash),
                candidate.base_domain_revision,
            ),
            _ => (
                candidate.candidate_id.clone(),
                candidate.semantic_hash.clone(),
                candidate.base_domain_revision + 1,
            ),
        };
        let event_count = core.events().len();
        let error = core
            .approve_spec(ApprovalCommand {
                session_id: before.session_id.clone(),
                expected_revision: before.revision,
                candidate_id,
                semantic_hash,
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

#[test]
fn candidate_provenance_seq_and_ordinal_mismatches_are_atomic() {
    let spec_candidate = |state: &PlanningState| {
        let audit_input_hash = state.full_audit.as_ref().unwrap().input_hash.clone();
        SpecCandidate {
            candidate_id: "candidate-provenance-spec".to_string(),
            created_event_seq: state.revision + 1,
            created_ordinal: 0,
            base_domain_revision: state.domain_revision,
            audit_input_hash,
            semantic_hash: crate::planning::engine::spec_semantic_hash(
                state,
                &serde_json::json!({"title":"spec"}),
            ),
            entity_refs: Vec::new(),
            content: serde_json::json!({"title":"spec"}),
            stale: false,
        }
    };
    for field in ["spec_seq", "spec_ordinal"] {
        let (mut control_core, control_state) = full_audit_core();
        let control_candidate = spec_candidate(&control_state);
        control_core
            .generate_spec(SpecCandidateCommand {
                session_id: control_state.session_id.clone(),
                expected_revision: control_state.revision,
                candidate: control_candidate,
            })
            .unwrap();

        let (mut core, state) = full_audit_core();
        let mut candidate = spec_candidate(&state);
        if field.ends_with("seq") {
            candidate.created_event_seq += 1;
        } else {
            candidate.created_ordinal = 1;
        }
        let before = state.clone();
        let event_count = core.events().len();
        let error = core
            .generate_spec(SpecCandidateCommand {
                session_id: state.session_id.clone(),
                expected_revision: state.revision,
                candidate,
            })
            .unwrap_err();
        assert!(matches!(error, CoreError::ProposalBaseMismatch), "{field}");
        assert_eq!(core.events().len(), event_count);
        assert_eq!(core.state(&state.session_id).unwrap(), &before);
    }

    let plan_candidate = |state: &PlanningState| {
        let work = state.required_model_action.as_ref().unwrap();
        let requirement = state.entities.current_requirements()[0];
        let criterion = state.entities.current_acceptance_criteria()[0];
        let content = serde_json::json!({
            "baseline":{"commands":["cargo test"],"known_failure_policy":"stop"},
            "steps":[{"temp_ref":"step-1","objective":"검증한다.","requirement_refs":[{"id":requirement.entity_id,"revision":requirement.revision}],"depends_on":[],"change_surface":["src"],"risks":[],"rollback_or_recovery":"복구한다."}],
            "verifications":[{"temp_ref":"verify-1","acceptance_criterion_ref":{"id":criterion.entity_id,"revision":criterion.revision},"plan_step_refs":["step-1"],"method":"command","procedure":"cargo test","expected_result":"통과한다."}],
            "plan_risks":[]
        });
        let spec_approval = state.spec.approval.as_ref().unwrap();
        PlanCandidate {
            candidate_id: "candidate-provenance-plan".to_string(),
            created_event_seq: state.revision + 1,
            created_ordinal: 0,
            base_plan_revision: state.plan_revision,
            plan_input_hash: work.input_hash.clone(),
            semantic_hash: crate::planning::canonical::canonical_hash(&content),
            spec_candidate_id: spec_approval.candidate_id.clone(),
            spec_semantic_hash: spec_approval.semantic_hash.clone(),
            content,
            stale: false,
        }
    };
    for field in ["plan_seq", "plan_ordinal"] {
        let (mut control_core, control_state) = approved_spec_core();
        let control_candidate = plan_candidate(&control_state);
        control_core
            .generate_plan(PlanCandidateCommand {
                session_id: control_state.session_id.clone(),
                expected_revision: control_state.revision,
                candidate: control_candidate,
            })
            .unwrap();

        let (mut core, state) = approved_spec_core();
        let mut candidate = plan_candidate(&state);
        if field.ends_with("seq") {
            candidate.created_event_seq += 1;
        } else {
            candidate.created_ordinal = 1;
        }
        let before = state.clone();
        let event_count = core.events().len();
        let error = core
            .generate_plan(PlanCandidateCommand {
                session_id: state.session_id.clone(),
                expected_revision: state.revision,
                candidate,
            })
            .unwrap_err();
        assert!(matches!(error, CoreError::ProposalBaseMismatch), "{field}");
        assert_eq!(core.events().len(), event_count);
        assert_eq!(core.state(&state.session_id).unwrap(), &before);
    }
}
