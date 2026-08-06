use crate::planning::{domain::*, engine::*};
use crate::planning_support::*;

#[test]
fn domain_invalidation_revokes_spec_and_plan_approval() {
    let (mut core, complete) = completed_core();
    let invalidated = core
        .revise_spec(RevisionRequestCommand {
            session_id: complete.session_id.clone(),
            expected_revision: complete.revision,
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
    let (mut core, approved_spec) = approved_spec_core();
    let requirement = approved_spec.entities.current_requirements()[0];
    let criterion = approved_spec.entities.current_acceptance_criteria()[0];
    let content = serde_json::json!({
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
    let plan_hash = crate::planning::canonical::canonical_hash(&content);
    let plan_work = approved_spec.required_model_action.as_ref().unwrap();
    let spec_approval = approved_spec.spec.approval.as_ref().unwrap();
    let generated_plan = core
        .generate_plan(PlanCandidateCommand {
            session_id: approved_spec.session_id.clone(),
            expected_revision: approved_spec.revision,
            candidate: PlanCandidate {
                candidate_id: "cand_plan".to_string(),
                created_event_seq: approved_spec.revision + 1,
                created_ordinal: 0,
                base_plan_revision: approved_spec.plan_revision,
                plan_input_hash: plan_work.input_hash.clone(),
                semantic_hash: plan_hash,
                spec_candidate_id: spec_approval.candidate_id.clone(),
                spec_semantic_hash: spec_approval.semantic_hash.clone(),
                content,
                stale: false,
            },
        })
        .unwrap();
    let revised = core
        .revise_plan(RevisionRequestCommand {
            session_id: generated_plan.state.session_id.clone(),
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
    let work_item = revised.state.required_model_action.as_ref().unwrap();
    assert_eq!(
        work_item.context["revision_feedback"][0]["text"],
        "검증 절차를 다시 쓴다."
    );
    assert_eq!(
        work_item.input_hash,
        crate::planning::engine::plan_input_hash(&revised.state)
    );
}

#[test]
fn spec_revision_feedback_is_retained_in_the_next_audit_work_item() {
    let (mut core, generated) = generated_spec_core();
    let revised = core
        .revise_spec(RevisionRequestCommand {
            session_id: generated.session_id.clone(),
            expected_revision: generated.revision,
            candidate_id: "cand_spec".to_string(),
            text: "성공 기준을 더 명확히 한다.".to_string(),
        })
        .unwrap();
    let work_item = revised.state.required_model_action.as_ref().unwrap();
    assert_eq!(work_item.kind, ModelActionKind::DeltaAudit);
    assert_eq!(
        work_item.context["revision_feedback"][0]["text"],
        "성공 기준을 더 명확히 한다."
    );
    assert_eq!(
        revised.state.transcript.revision_feedback[0].candidate_id,
        "cand_spec"
    );
}

#[test]
fn identical_evidence_refresh_is_a_noop_without_event_or_revision() {
    let (mut core, state) = start_core();
    let first = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            snapshot: snapshot("sha256:evidence-a"),
        })
        .unwrap();
    let changed = match first {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("first snapshot must change state"),
    };
    let event_count = core.events().len();
    let second = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: changed.session_id.clone(),
            expected_revision: changed.revision,
            snapshot: snapshot("sha256:evidence-a"),
        })
        .unwrap();
    let unchanged = match second {
        EvidenceRefreshResult::Unchanged { state } => state,
        EvidenceRefreshResult::Changed(_) => panic!("identical snapshot must be a no-op"),
    };
    assert_eq!(unchanged, changed);
    assert_eq!(core.events().len(), event_count);
}

fn evidence_dependency_core() -> (InMemoryPlanningCore, PlanningState) {
    let (mut core, state) = start_core();
    let refreshed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            snapshot: snapshot("sha256:evidence-a"),
        })
        .unwrap();
    let state = match refreshed {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("initial snapshot must change state"),
    };
    let work_item = state.required_model_action.clone().unwrap();
    let source = || {
        vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }]
    };
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
            entity_ops: vec![
                EntityOp::Create {
                    temp_ref: "fact".to_string(),
                    body: EntityBody::Fact {
                        statement: "저장소 사실".to_string(),
                        evidence_refs: vec!["EVID-001".to_string()],
                    },
                    source_refs: vec![SourceRef::Evidence {
                        id: "EVID-001".to_string(),
                    }],
                },
                EntityOp::Create {
                    temp_ref: "constraint".to_string(),
                    body: EntityBody::Constraint {
                        statement: "사실에 의존하는 제약".to_string(),
                    },
                    source_refs: source(),
                },
                EntityOp::Create {
                    temp_ref: "risk".to_string(),
                    body: EntityBody::Risk {
                        statement: "파생 위험".to_string(),
                        impact: RiskImpact::Medium,
                        mitigation: "다시 확인한다.".to_string(),
                    },
                    source_refs: source(),
                },
                EntityOp::Create {
                    temp_ref: "unrelated".to_string(),
                    body: EntityBody::Problem {
                        statement: "독립적인 사용자 문제".to_string(),
                    },
                    source_refs: source(),
                },
            ],
            edge_ops: vec![
                EdgeOp::Add {
                    kind: EdgeKind::DependsOn,
                    from: AuditEndpoint::TempRef {
                        temp_ref: "constraint".to_string(),
                    },
                    to: AuditEndpoint::TempRef {
                        temp_ref: "fact".to_string(),
                    },
                    source_refs: source(),
                },
                EdgeOp::Add {
                    kind: EdgeKind::DependsOn,
                    from: AuditEndpoint::TempRef {
                        temp_ref: "risk".to_string(),
                    },
                    to: AuditEndpoint::TempRef {
                        temp_ref: "constraint".to_string(),
                    },
                    source_refs: source(),
                },
            ],
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    (core, audit.state)
}

#[test]
fn changed_evidence_stales_transitive_dependents_and_preserves_unrelated_graph() {
    let (mut core, before) = evidence_dependency_core();
    let event_count = core.events().len();
    let old_edges = before.entities.edges.clone();
    let changed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            snapshot: snapshot("sha256:evidence-b"),
        })
        .unwrap();
    let result = match changed {
        EvidenceRefreshResult::Changed(result) => result,
        EvidenceRefreshResult::Unchanged { .. } => panic!("changed snapshot must mutate state"),
    };
    assert_eq!(result.state.revision, before.revision + 1);
    assert_eq!(result.state.domain_revision, before.domain_revision + 1);
    assert_eq!(result.state.phase, LifecyclePhase::Interview);
    assert!(result.state.pending_question.is_none());
    assert_eq!(result.state.full_audit, None);
    assert_eq!(
        result.state.required_model_action.as_ref().unwrap().kind,
        ModelActionKind::DeltaAudit
    );
    assert_eq!(result.state.entities.edges, old_edges);
    for entity_id in ["FACT-001", "CON-001", "RISK-001"] {
        let entity = result.state.entities.at_revision(entity_id, 1).unwrap();
        assert!(matches!(entity.validity, EntityValidity::Stale { .. }));
    }
    assert!(result
        .state
        .entities
        .at_revision("PROB-001", 1)
        .unwrap()
        .is_current());
    assert_eq!(core.events().len(), event_count + 1);
    for entity_id in ["FACT-001", "CON-001", "RISK-001"] {
        assert!(result.event.effects.iter().any(|effect| {
            matches!(effect, EventEffect::EntityInvalidated { entity_id: id } if id == entity_id)
        }));
    }
}

#[test]
fn changed_evidence_cancels_pending_question_and_invalidates_complete_artifacts() {
    let (mut core, interview) = start_core();
    let work_item = interview.required_model_action.clone().unwrap();
    let pending = core
        .apply_audit(AuditCommand {
            session_id: interview.session_id.clone(),
            expected_revision: interview.revision,
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
            counterexample_review: None,
        })
        .unwrap();
    let changed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: pending.state.session_id.clone(),
            expected_revision: pending.state.revision,
            snapshot: snapshot("sha256:evidence-new"),
        })
        .unwrap();
    let changed = match changed {
        EvidenceRefreshResult::Changed(result) => result,
        EvidenceRefreshResult::Unchanged { .. } => panic!("changed snapshot must mutate state"),
    };
    assert!(changed.state.pending_question.is_none());
    assert_eq!(
        changed.state.required_model_action.as_ref().unwrap().kind,
        ModelActionKind::DeltaAudit
    );

    let (mut core, complete) = completed_core();
    assert!(complete.spec.approval.is_some());
    assert!(complete.plan.approval.is_some());
    let changed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: complete.session_id.clone(),
            expected_revision: complete.revision,
            snapshot: snapshot("sha256:evidence-new"),
        })
        .unwrap();
    let changed = match changed {
        EvidenceRefreshResult::Changed(result) => result,
        EvidenceRefreshResult::Unchanged { .. } => panic!("changed snapshot must mutate state"),
    };
    assert_eq!(changed.state.phase, LifecyclePhase::Interview);
    assert!(changed.state.full_audit.is_none());
    assert!(changed.state.spec.approval.is_none());
    assert!(changed.state.plan.approval.is_none());
    assert!(changed.state.spec.current_candidate.as_ref().unwrap().stale);
    assert!(changed.state.plan.current_candidate.as_ref().unwrap().stale);
    assert_eq!(
        changed.state.required_model_action.as_ref().unwrap().kind,
        ModelActionKind::DeltaAudit
    );
}

#[test]
fn stale_entity_recovery_supersedes_stale_revision_with_new_valid_revision() {
    let (mut core, before) = evidence_dependency_core();
    let refreshed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            snapshot: snapshot("sha256:evidence-b"),
        })
        .unwrap();
    let stale = match refreshed {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("changed snapshot must mutate state"),
    };
    let old = stale.entities.at_revision("FACT-001", 1).unwrap().clone();
    let work_item = stale.required_model_action.clone().unwrap();
    let recovered = core
        .apply_audit(AuditCommand {
            session_id: stale.session_id.clone(),
            expected_revision: stale.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Revise {
                entity_id: "FACT-001".to_string(),
                base_entity_revision: 1,
                body: EntityBody::Fact {
                    statement: "갱신된 저장소 사실".to_string(),
                    evidence_refs: vec!["EVID-002".to_string()],
                },
                source_refs: vec![SourceRef::Evidence {
                    id: "EVID-002".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let old_after = recovered.state.entities.at_revision("FACT-001", 1).unwrap();
    assert_eq!(old_after.validity, old.validity);
    assert_eq!(old_after.disposition, EntityDisposition::Superseded);
    let new = recovered.state.entities.at_revision("FACT-001", 2).unwrap();
    assert_eq!(new.disposition, EntityDisposition::Current);
    assert_eq!(new.validity, EntityValidity::Valid);
    assert_eq!(recovered.state.entities.current("FACT-001"), Some(new));
}

#[test]
fn rejected_latest_entity_cannot_be_revised_and_failure_is_atomic() {
    let (mut core, state) = start_core();
    let work_item = state.required_model_action.clone().unwrap();
    let created = core
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
                temp_ref: "problem".to_string(),
                body: EntityBody::Problem {
                    statement: "거부할 문제".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let full_work = created.state.required_model_action.clone().unwrap();
    let rejected = core
        .apply_audit(AuditCommand {
            session_id: created.state.session_id.clone(),
            expected_revision: created.state.revision,
            work_item_id: full_work.work_item_id,
            mode: AuditMode::Full,
            base_revision: full_work.base_revision,
            base_domain_revision: full_work.base_domain_revision,
            input_hash: full_work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: vec![EntityOp::Reject {
                entity_id: "PROB-001".to_string(),
                base_entity_revision: 1,
                reason: "근거 부족".to_string(),
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    let before = rejected.state.clone();
    let event_count = core.events().len();
    let work_item = before.required_model_action.clone().unwrap();
    let error = core
        .apply_audit(AuditCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: work_item.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work_item.base_revision,
            base_domain_revision: work_item.base_domain_revision,
            input_hash: work_item.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: vec![EntityOp::Revise {
                entity_id: "PROB-001".to_string(),
                base_entity_revision: 2,
                body: EntityBody::Problem {
                    statement: "되살리면 안 된다".to_string(),
                },
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }],
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap_err();
    assert!(matches!(error, CoreError::ProposalSchemaInvalid(_)));
    assert_eq!(core.events().len(), event_count);
    assert_eq!(core.state(&before.session_id).unwrap(), &before);
}
