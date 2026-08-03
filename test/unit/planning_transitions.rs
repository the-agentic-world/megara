use crate::planning::{domain::*, engine::*};
use crate::planning_support::*;

type PlanningFixture = fn() -> (InMemoryPlanningCore, PlanningState);

#[test]
fn spec_revision_transition_table_accepts_only_specification_and_complete() {
    let cases: [(&str, PlanningFixture, bool); 4] = [
        ("interview", start_core, false),
        ("specification", generated_spec_core, true),
        ("planning", approved_spec_core, false),
        ("complete", completed_core, true),
    ];

    for (label, fixture, legal) in cases {
        let (mut core, before) = fixture();
        let event_count = core.events().len();
        let result = core.revise_spec(RevisionRequestCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            candidate_id: "cand_spec".to_string(),
            text: format!("{label} transition"),
        });
        assert_eq!(
            result.is_ok(),
            legal,
            "unexpected transition result: {label}"
        );
        if legal {
            assert_eq!(result.unwrap().state.phase, LifecyclePhase::Interview);
        } else {
            assert!(matches!(result, Err(CoreError::InvalidPhase(_))));
            assert_eq!(core.events().len(), event_count);
            assert_eq!(core.state(&before.session_id).unwrap(), &before);
        }
    }
}

#[derive(Clone, Copy)]
enum Operation {
    AuditDelta,
    AuditFull,
    Answer,
    SpecGenerate,
    SpecApprove,
    PlanGenerate,
    PlanApprove,
    PlanRevise,
    SpecRevise,
}

fn core_at_phase(phase: LifecyclePhase) -> (InMemoryPlanningCore, PlanningState) {
    match phase {
        LifecyclePhase::Interview => start_core(),
        LifecyclePhase::Specification => generated_spec_core(),
        LifecyclePhase::Planning => approved_spec_core(),
        LifecyclePhase::Complete => completed_core(),
    }
}

#[test]
fn transition_matrix_lists_allowed_phases_and_rejects_each_illegal_phase_atomically() {
    let matrix: [(&str, Operation, &[LifecyclePhase]); 9] = [
        (
            "audit delta",
            Operation::AuditDelta,
            &[LifecyclePhase::Interview],
        ),
        (
            "audit full",
            Operation::AuditFull,
            &[LifecyclePhase::Interview],
        ),
        ("answer", Operation::Answer, &[LifecyclePhase::Interview]),
        (
            "spec generate",
            Operation::SpecGenerate,
            &[LifecyclePhase::Specification],
        ),
        (
            "spec approve",
            Operation::SpecApprove,
            &[LifecyclePhase::Specification],
        ),
        (
            "plan generate",
            Operation::PlanGenerate,
            &[LifecyclePhase::Planning],
        ),
        (
            "plan approve",
            Operation::PlanApprove,
            &[LifecyclePhase::Planning],
        ),
        (
            "plan revise",
            Operation::PlanRevise,
            &[LifecyclePhase::Planning],
        ),
        (
            "spec revise",
            Operation::SpecRevise,
            &[LifecyclePhase::Specification, LifecyclePhase::Complete],
        ),
    ];

    let phases = [
        LifecyclePhase::Interview,
        LifecyclePhase::Specification,
        LifecyclePhase::Planning,
        LifecyclePhase::Complete,
    ];
    for (label, operation, allowed_phases) in matrix {
        for phase in phases {
            if allowed_phases.contains(&phase) {
                continue;
            }
            let (mut core, before) = core_at_phase(phase);
            let event_count = core.events().len();
            let result = match operation {
                Operation::AuditDelta | Operation::AuditFull => core
                    .apply_audit(AuditCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        work_item_id: "wrk_illegal".to_string(),
                        mode: if matches!(operation, Operation::AuditDelta) {
                            AuditMode::Delta
                        } else {
                            AuditMode::Full
                        },
                        base_revision: before.revision,
                        base_domain_revision: before.domain_revision,
                        input_hash: "sha256:illegal".to_string(),
                        readiness: AuditReadiness::RequestFullAudit,
                        next_question: None,
                        entity_ops: Vec::new(),
                        edge_ops: Vec::new(),
                        blocker_ops: Vec::new(),
                        counterexample_review: Some(CounterexampleReview::performed()),
                    })
                    .map(|_| ()),
                Operation::Answer => core
                    .answer(AnswerCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        question_id: "qst_missing".to_string(),
                        based_on_revision: before.revision,
                        text: "답".to_string(),
                        selected_choice_ids: Vec::new(),
                    })
                    .map(|_| ()),
                Operation::SpecGenerate => core
                    .generate_spec(SpecCandidateCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate: SpecCandidate {
                            candidate_id: "cand_spec".to_string(),
                            created_event_seq: before.revision + 1,
                            created_ordinal: 0,
                            base_domain_revision: before.domain_revision,
                            audit_input_hash: "sha256:audit".to_string(),
                            semantic_hash: "sha256:spec".to_string(),
                            entity_refs: Vec::new(),
                            content: serde_json::json!({}),
                            stale: false,
                        },
                    })
                    .map(|_| ()),
                Operation::SpecApprove => core
                    .approve_spec(ApprovalCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate_id: "cand_spec".to_string(),
                        semantic_hash: "sha256:spec".to_string(),
                        base_revision: before.domain_revision,
                    })
                    .map(|_| ()),
                Operation::PlanGenerate => core
                    .generate_plan(PlanCandidateCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate: PlanCandidate {
                            candidate_id: "cand_plan".to_string(),
                            created_event_seq: before.revision + 1,
                            created_ordinal: 0,
                            base_plan_revision: before.plan_revision,
                            plan_input_hash: "sha256:plan-input".to_string(),
                            semantic_hash: "sha256:plan".to_string(),
                            spec_candidate_id: "cand_spec".to_string(),
                            spec_semantic_hash: "sha256:spec".to_string(),
                            content: serde_json::json!({}),
                            stale: false,
                        },
                    })
                    .map(|_| ()),
                Operation::PlanApprove => core
                    .approve_plan(ApprovalCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate_id: "cand_plan".to_string(),
                        semantic_hash: "sha256:plan".to_string(),
                        base_revision: before.plan_revision,
                    })
                    .map(|_| ()),
                Operation::PlanRevise => core
                    .revise_plan(RevisionRequestCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate_id: "cand_plan".to_string(),
                        text: "수정".to_string(),
                    })
                    .map(|_| ()),
                Operation::SpecRevise => core
                    .revise_spec(RevisionRequestCommand {
                        session_id: before.session_id.clone(),
                        expected_revision: before.revision,
                        candidate_id: "cand_spec".to_string(),
                        text: "수정".to_string(),
                    })
                    .map(|_| ()),
            };
            assert!(
                result.is_err(),
                "illegal transition unexpectedly succeeded: {label}"
            );
            assert!(
                matches!(result, Err(CoreError::InvalidPhase(_))),
                "wrong error for illegal transition: {label}"
            );
            assert_eq!(core.events().len(), event_count, "event changed: {label}");
            assert_eq!(
                core.state(&before.session_id).unwrap(),
                &before,
                "state changed: {label}"
            );
        }
    }
}

#[test]
fn evidence_refresh_is_allowed_from_every_lifecycle_phase() {
    let fixtures: [(&str, PlanningFixture); 4] = [
        ("interview", start_core),
        ("specification", generated_spec_core),
        ("planning", approved_spec_core),
        ("complete", completed_core),
    ];
    for (label, fixture) in fixtures {
        let (mut core, before) = fixture();
        let result = core
            .refresh_evidence(EvidenceRefreshCommand {
                session_id: before.session_id.clone(),
                expected_revision: before.revision,
                snapshot: snapshot(&format!("sha256:{label}-changed")),
            })
            .unwrap();
        let changed = match result {
            EvidenceRefreshResult::Changed(result) => result,
            EvidenceRefreshResult::Unchanged { .. } => {
                panic!("changed snapshot was a no-op: {label}")
            }
        };
        assert_eq!(changed.state.phase, LifecyclePhase::Interview);
        assert_eq!(changed.state.revision, before.revision + 1);
    }
}
