use super::planning::{domain::*, engine::*};

fn start_core() -> (InMemoryPlanningCore, PlanningState) {
    let mut core = InMemoryPlanningCore::default();
    let result = core
        .start(StartCommand {
            session_id: Some("pln_1".to_string()),
            project_id: "prj_1".to_string(),
            request: "기획 상태를 저장한다.".to_string(),
        })
        .unwrap();
    (core, result.state)
}

fn question() -> QuestionProposal {
    QuestionProposal {
        context: "결정 배경입니다.".to_string(),
        question: "어떤 결과를 원하시나요?".to_string(),
        why_it_matters: "결과에 따라 명세가 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Freeform {
            freeform_hint: "원하는 결과를 설명해 주세요.".to_string(),
        },
    }
}

fn required_entity_ops() -> (Vec<EntityOp>, Vec<EdgeOp>) {
    let source = || {
        vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }]
    };
    let entities = vec![
        EntityOp::Create {
            temp_ref: "problem".to_string(),
            body: EntityBody::Problem {
                statement: "문제를 설명한다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "outcome".to_string(),
            body: EntityBody::Outcome {
                statement: "결과를 설명한다.".to_string(),
                observable_result: "검토 가능한 결과가 있다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "requirement".to_string(),
            body: EntityBody::Requirement {
                statement: "요구사항을 보존한다.".to_string(),
                priority: RequirementPriority::Must,
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "non_goal".to_string(),
            body: EntityBody::NonGoal {
                statement: "구현 실행은 하지 않는다.".to_string(),
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "boundary".to_string(),
            body: EntityBody::DecisionBoundary {
                autonomous_scope: vec!["구조 검증".to_string()],
                requires_user_approval: vec!["승인".to_string()],
            },
            source_refs: source(),
        },
        EntityOp::Create {
            temp_ref: "criterion".to_string(),
            body: EntityBody::AcceptanceCriterion {
                statement: "요구사항을 확인할 수 있다.".to_string(),
            },
            source_refs: source(),
        },
    ];
    let edges = vec![EdgeOp {
        kind: EdgeKind::HasAcceptanceCriterion,
        from: AuditEndpoint::TempRef("requirement".to_string()),
        to: AuditEndpoint::TempRef("criterion".to_string()),
        source_refs: source(),
    }];
    (entities, edges)
}

#[test]
fn start_creates_one_revision_and_one_model_action() {
    let (core, state) = start_core();
    assert_eq!(state.revision, 1);
    assert_eq!(core.events().len(), 1);
    assert_eq!(core.events()[0].seq, 1);
    assert_eq!(state.domain_revision, 1);
    assert_eq!(core.events()[0].domain_revision_after, 1);
    assert_eq!(
        state.required_model_action.as_ref().unwrap().base_revision,
        1
    );
    assert_eq!(
        state
            .required_model_action
            .as_ref()
            .unwrap()
            .base_domain_revision,
        1
    );
    assert!(state.derived().waiting_for_model);
    assert!(!state.derived().waiting_for_user);
}

#[test]
fn stale_answer_has_no_event_or_state_change() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.unwrap();
    let audit = AuditCommand {
        session_id: state.session_id.clone(),
        expected_revision: state.revision,
        work_item_id: work_item.work_item_id,
        mode: AuditMode::Delta,
        base_revision: work_item.base_revision,
        base_domain_revision: work_item.base_domain_revision,
        input_hash: work_item.input_hash,
        readiness: AuditReadiness::Continue,
        next_question: Some(question()),
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review_performed: false,
    };
    let result = core.apply_audit(audit).unwrap();
    let before = result.state.clone();
    let event_count = core.events().len();
    let error = core
        .answer(AnswerCommand {
            session_id: "pln_1".to_string(),
            expected_revision: before.revision - 1,
            question_id: "qst_2".to_string(),
            based_on_revision: before.revision,
            text: "답".to_string(),
            selected_choice_ids: Vec::new(),
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::RevisionConflict { .. }));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state("pln_1").unwrap(), &before);
}

#[test]
fn pending_question_and_model_action_never_coexist() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.unwrap();
    let result = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap();
    assert!(result.state.pending_question.is_some());
    assert!(result.state.required_model_action.is_none());
    assert!(result.state.assert_invariants().is_ok());
}

fn generated_spec_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, state) = start_core();
    core.seed_repo_snapshot_for_test("pln_1");
    let work_item = state.required_model_action.unwrap();
    let (entity_ops, edge_ops) = required_entity_ops();
    let audit = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops,
            edge_ops,
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap();
    let full_work = audit.state.required_model_action.unwrap();
    let full = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: audit.state.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash.clone(),
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: true,
        })
        .unwrap();
    let generated = core
        .generate_spec(SpecCandidateCommand {
            session_id: state.session_id.clone(),
            expected_revision: full.state.revision,
            candidate: SpecCandidate {
                candidate_id: "cand_spec".to_string(),
                base_domain_revision: full.state.domain_revision,
                audit_input_hash: full_work.input_hash,
                semantic_hash: "sha256:spec".to_string(),
                entity_refs: Vec::new(),
                content: serde_json::json!({"title":"spec"}),
                stale: false,
            },
        })
        .unwrap();
    (core, generated.state)
}

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
    let plan_work = approved.state.required_model_action.unwrap();
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
        ("cand_spec", "sha256:spec", 3),
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

#[test]
fn core_does_not_enter_specification_from_self_attested_empty_gate() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.unwrap();
    let requested = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap();
    let before = requested.state.clone();
    let event_count = core.events().len();
    let full_work = before.required_model_action.clone().unwrap();
    let error = core
        .apply_audit(AuditCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash,
            readiness: AuditReadiness::Ready,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: true,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::ProposalSchemaInvalid(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}

#[test]
fn domain_invalidation_revokes_spec_and_plan_approval() {
    let (mut core, generated) = generated_spec_core();
    let approved_spec = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash: "sha256:spec".to_string(),
            base_revision: generated.domain_revision,
        })
        .unwrap();
    let plan_work = approved_spec.state.required_model_action.clone().unwrap();
    let generated_plan = core
        .generate_plan(PlanCandidateCommand {
            session_id: generated.session_id.clone(),
            expected_revision: approved_spec.state.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                base_plan_revision: approved_spec.state.plan_revision,
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
    let complete = core
        .approve_plan(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated_plan.state.revision,
            candidate_id: "cand_plan".to_string(),
            semantic_hash: "sha256:plan".to_string(),
            base_revision: generated_plan.state.plan_revision,
        })
        .unwrap();
    let invalidated = core
        .revise_spec(RevisionRequestCommand {
            session_id: generated.session_id.clone(),
            expected_revision: complete.state.revision,
            candidate_id: "cand_spec".to_string(),
            text: "사용자 의미를 다시 확인한다.".to_string(),
        })
        .unwrap();
    assert_eq!(invalidated.state.phase, LifecyclePhase::Interview);
    assert!(invalidated.state.spec.approval.is_none());
    assert!(invalidated.state.plan.approval.is_none());
    assert!(
        invalidated
            .state
            .spec
            .current_candidate
            .as_ref()
            .unwrap()
            .stale
    );
    assert!(
        invalidated
            .state
            .plan
            .current_candidate
            .as_ref()
            .unwrap()
            .stale
    );
}

#[test]
fn plan_only_invalidation_preserves_spec_approval() {
    let (mut core, generated) = generated_spec_core();
    let approved_spec = core
        .approve_spec(ApprovalCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            semantic_hash: "sha256:spec".to_string(),
            base_revision: generated.domain_revision,
        })
        .unwrap();
    let generated_plan = core
        .generate_plan(PlanCandidateCommand {
            session_id: generated.session_id.clone(),
            expected_revision: approved_spec.state.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                base_plan_revision: approved_spec.state.plan_revision,
                plan_input_hash: "sha256:plan-input".to_string(),
                semantic_hash: "sha256:plan".to_string(),
                spec_candidate_id: "cand_spec".to_string(),
                spec_semantic_hash: "sha256:spec".to_string(),
                content: serde_json::json!({"steps": []}),
                stale: false,
            },
        })
        .unwrap();
    let revised = core
        .revise_plan(RevisionRequestCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated_plan.state.revision,
            candidate_id: "cand_plan".to_string(),
            text: "검증 절차를 다시 쓴다.".to_string(),
        })
        .unwrap();
    assert!(revised.state.spec.approval.is_some());
    assert!(revised.state.plan.approval.is_none());
    assert!(revised.state.plan.current_candidate.as_ref().unwrap().stale);
    assert_eq!(
        revised.state.plan_revision,
        generated_plan.state.plan_revision + 1
    );
}

#[test]
fn source_rule_and_illegal_phase_failures_preserve_state() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.clone().unwrap();
    let before = state.clone();
    let event_count = core.events().len();
    let error = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "fact".to_string(),
                body: EntityBody::Fact {
                    statement: "비밀 사실".to_string(),
                    evidence_refs: vec!["EVID-001".to_string()],
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidRequest(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);

    let error = core
        .generate_spec(SpecCandidateCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            candidate: SpecCandidate {
                candidate_id: "cand_invalid".to_string(),
                base_domain_revision: 0,
                audit_input_hash: "sha256:none".to_string(),
                semantic_hash: "sha256:none".to_string(),
                entity_refs: Vec::new(),
                content: serde_json::json!({}),
                stale: false,
            },
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidPhase(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}

#[test]
fn requirement_cannot_use_historical_decision_source() {
    let (mut core, state) = start_core();
    let start_work = state.required_model_action.unwrap();
    let created = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: start_work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: start_work.base_revision,
            base_domain_revision: start_work.base_domain_revision,
            input_hash: start_work.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "decision".to_string(),
                body: EntityBody::Decision {
                    statement: "결정을 기록한다.".to_string(),
                    selected_option: "현재 선택".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap();
    let full_work = created.state.required_model_action.unwrap();
    let revised = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: created.state.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: vec![EntityOp::Revise {
                entity_id: "DEC-001".to_string(),
                base_revision: 1,
                body: EntityBody::Decision {
                    statement: "결정을 다시 기록한다.".to_string(),
                    selected_option: "새 선택".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: true,
        })
        .unwrap();
    let delta_work = revised.state.required_model_action.clone().unwrap();
    let before = revised.state.clone();
    let event_count = core.events().len();
    let error = core
        .apply_audit(AuditCommand {
            session_id: state.session_id,
            expected_revision: before.revision,
            work_item_id: delta_work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: delta_work.base_revision,
            base_domain_revision: delta_work.base_domain_revision,
            input_hash: delta_work.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Create {
                temp_ref: "requirement".to_string(),
                body: EntityBody::Requirement {
                    statement: "옛 결정을 근거로 삼는다.".to_string(),
                    priority: RequirementPriority::Must,
                },
                source_refs: vec![SourceRef::Entity {
                    id: "DEC-001".to_string(),
                    revision: 1,
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review_performed: false,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::InvalidRequest(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state("pln_1").unwrap(), &before);
}
