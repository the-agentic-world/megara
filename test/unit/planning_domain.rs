use super::planning::domain::*;

#[test]
fn lifecycle_has_only_four_phases_and_waiting_is_orthogonal() {
    let state = PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
    assert_eq!(state.phase, LifecyclePhase::Interview);
    assert!(!state.derived().waiting_for_user);
    assert!(!state.derived().waiting_for_model);
}

#[test]
fn pending_question_has_typed_answer_variant() {
    let question = QuestionProposal {
        context: "배경입니다.".to_string(),
        question: "무엇을 먼저 확인할까요?".to_string(),
        why_it_matters: "다음 단계가 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Choice {
            choices: vec![
                Choice {
                    id: "a".to_string(),
                    label: "첫 번째".to_string(),
                    direction: "첫 번째 방향으로 진행합니다.".to_string(),
                    benefits: vec!["확인 범위가 분명합니다.".to_string()],
                    tradeoffs: vec!["질문이 하나 더 필요할 수 있습니다.".to_string()],
                },
                Choice {
                    id: "b".to_string(),
                    label: "두 번째".to_string(),
                    direction: "두 번째 방향으로 진행합니다.".to_string(),
                    benefits: vec!["빠르게 좁힐 수 있습니다.".to_string()],
                    tradeoffs: vec!["기록을 다시 확인해야 할 수 있습니다.".to_string()],
                },
            ],
            recommendation: None,
            freeform_hint: "원하는 방향을 직접 설명해 주세요.".to_string(),
        },
    };
    let value = serde_json::to_value(question).unwrap();
    assert_eq!(value["answer"]["mode"], "choice");
}

#[test]
fn graph_rejects_dangling_wrong_duplicate_and_invalid_supersedes_edges() {
    let mut graph = EntityGraph::default();
    let requirement = EntityRecord {
        entity_id: "REQ-001".to_string(),
        internal_uuid: "00000000-0000-7000-8000-000000000001".to_string(),
        revision: 1,
        kind: EntityKind::Requirement,
        body: EntityBody::Requirement {
            statement: "상태를 저장한다.".to_string(),
            priority: RequirementPriority::Must,
        },
        disposition: EntityDisposition::Current,
        validity: EntityValidity::Valid,
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        created_event_seq: 1,
        created_ordinal: 0,
    };
    graph.insert(requirement).unwrap();
    graph
        .insert(EntityRecord {
            entity_id: "AC-001".to_string(),
            internal_uuid: "00000000-0000-7000-8000-000000000002".to_string(),
            revision: 1,
            kind: EntityKind::AcceptanceCriterion,
            body: EntityBody::AcceptanceCriterion {
                statement: "확인한다.".to_string(),
            },
            disposition: EntityDisposition::Current,
            validity: EntityValidity::Valid,
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            created_event_seq: 1,
            created_ordinal: 1,
        })
        .unwrap();
    let source_refs = vec![SourceRef::InitialRequest {
        id: "request".to_string(),
    }];
    let dangling = Edge {
        edge_id: "edge-dangling".to_string(),
        revision: 1,
        kind: EdgeKind::HasAcceptanceCriterion,
        from: EntityRef {
            id: "REQ-001".to_string(),
            revision: 1,
        },
        to: EdgeTarget::Entity(EntityRef {
            id: "AC-404".to_string(),
            revision: 1,
        }),
        source_refs: source_refs.clone(),
        retired: false,
    };
    assert!(graph.add_edge(dangling).is_err());
    assert!(graph.edges.is_empty());

    let wrong_direction = Edge {
        edge_id: "edge-wrong-kind".to_string(),
        revision: 1,
        kind: EdgeKind::Implements,
        from: EntityRef {
            id: "REQ-001".to_string(),
            revision: 1,
        },
        to: EdgeTarget::Entity(EntityRef {
            id: "AC-001".to_string(),
            revision: 1,
        }),
        source_refs: source_refs.clone(),
        retired: false,
    };
    assert!(graph.add_edge(wrong_direction).is_err());
    assert!(graph.edges.is_empty());

    let valid = Edge {
        edge_id: "edge-valid".to_string(),
        revision: 1,
        kind: EdgeKind::HasAcceptanceCriterion,
        from: EntityRef {
            id: "REQ-001".to_string(),
            revision: 1,
        },
        to: EdgeTarget::Entity(EntityRef {
            id: "AC-001".to_string(),
            revision: 1,
        }),
        source_refs: source_refs.clone(),
        retired: false,
    };
    graph.add_edge(valid.clone()).unwrap();
    assert_eq!(graph.edges.len(), 1);
    let mut duplicate = valid;
    duplicate.edge_id = "edge-duplicate".to_string();
    assert!(graph.add_edge(duplicate).is_err());
    assert_eq!(graph.edges.len(), 1);

    let invalid_supersedes = Edge {
        edge_id: "edge-invalid-supersedes".to_string(),
        revision: 1,
        kind: EdgeKind::Supersedes,
        from: EntityRef {
            id: "REQ-001".to_string(),
            revision: 1,
        },
        to: EdgeTarget::Entity(EntityRef {
            id: "AC-001".to_string(),
            revision: 1,
        }),
        source_refs,
        retired: false,
    };
    assert!(graph.add_edge(invalid_supersedes).is_err());
    assert_eq!(graph.edges.len(), 1);
}

#[test]
fn entity_revision_must_exceed_history_even_after_stale_or_rejected_latest() {
    let source_refs = || {
        vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }]
    };
    let record = |entity_id: &str,
                  revision: u64,
                  disposition: EntityDisposition,
                  validity: EntityValidity| EntityRecord {
        entity_id: entity_id.to_string(),
        internal_uuid: format!("00000000-0000-7000-8000-{revision:012}"),
        revision,
        kind: EntityKind::Fact,
        body: EntityBody::Fact {
            statement: format!("fact {revision}"),
            evidence_refs: vec!["citation".to_string()],
        },
        disposition,
        validity,
        source_refs: source_refs(),
        created_event_seq: revision,
        created_ordinal: 0,
    };

    let mut stale_graph = EntityGraph::default();
    stale_graph
        .insert(record(
            "FACT-STALE",
            1,
            EntityDisposition::Current,
            EntityValidity::Valid,
        ))
        .unwrap();
    stale_graph
        .insert(record(
            "FACT-STALE",
            3,
            EntityDisposition::Current,
            EntityValidity::Stale {
                since_domain_revision: 3,
                causes: source_refs(),
            },
        ))
        .unwrap();
    let before_stale = stale_graph.clone();
    assert!(stale_graph
        .insert(record(
            "FACT-STALE",
            2,
            EntityDisposition::Current,
            EntityValidity::Valid,
        ))
        .is_err());
    assert_eq!(stale_graph, before_stale);

    let mut rejected_graph = EntityGraph::default();
    rejected_graph
        .insert(record(
            "FACT-REJECTED",
            1,
            EntityDisposition::Current,
            EntityValidity::Valid,
        ))
        .unwrap();
    rejected_graph
        .insert(record(
            "FACT-REJECTED",
            3,
            EntityDisposition::Rejected,
            EntityValidity::Valid,
        ))
        .unwrap();
    let before_rejected = rejected_graph.clone();
    assert!(rejected_graph
        .insert(record(
            "FACT-REJECTED",
            2,
            EntityDisposition::Current,
            EntityValidity::Valid,
        ))
        .is_err());
    assert_eq!(rejected_graph, before_rejected);
}

#[test]
fn derived_state_table_keeps_phase_waiting_blocker_approval_and_stale_orthogonal() {
    let phases = [
        LifecyclePhase::Interview,
        LifecyclePhase::Specification,
        LifecyclePhase::Planning,
        LifecyclePhase::Complete,
    ];
    let pending = PendingQuestion {
        question_id: "qst_1".to_string(),
        created_event_seq: 1,
        created_ordinal: 0,
        based_on_revision: 1,
        proposal: QuestionProposal {
            context: "배경".to_string(),
            question: "질문".to_string(),
            why_it_matters: "영향".to_string(),
            technical_terms: Vec::new(),
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            answer: AnswerMode::Freeform {
                freeform_hint: "답".to_string(),
            },
        },
    };
    let model_action = ModelWorkItem {
        kind: ModelActionKind::DeltaAudit,
        work_item_id: "wrk_1".to_string(),
        created_event_seq: 1,
        created_ordinal: 0,
        session_id: "pln_1".to_string(),
        base_revision: 1,
        base_domain_revision: 1,
        base_plan_revision: 0,
        input_hash: "sha256:input".to_string(),
        output_schema: "megara.audit-proposal/v1".to_string(),
        context: serde_json::json!({}),
    };

    for phase in phases {
        let mut user_waiting =
            PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
        user_waiting.phase = phase;
        user_waiting.pending_question = Some(pending.clone());
        let derived = user_waiting.derived();
        assert!(derived.waiting_for_user);
        assert!(!derived.waiting_for_model);

        let mut model_waiting = user_waiting.clone();
        model_waiting.pending_question = None;
        model_waiting.required_model_action = Some(model_action.clone());
        let derived = model_waiting.derived();
        assert!(!derived.waiting_for_user);
        assert!(derived.waiting_for_model);
    }

    let candidate = SpecCandidate {
        candidate_id: "cand_spec".to_string(),
        base_domain_revision: 1,
        audit_input_hash: "sha256:audit".to_string(),
        semantic_hash: "sha256:spec".to_string(),
        entity_refs: Vec::new(),
        content: serde_json::json!({}),
        stale: false,
    };
    let mut cases = Vec::new();
    let mut stale_candidate =
        PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
    stale_candidate.phase = LifecyclePhase::Specification;
    let mut stale = candidate.clone();
    stale.stale = true;
    stale_candidate.spec.current_candidate = Some(stale);
    cases.push(("stale candidate", stale_candidate, true, false));

    let mut mismatched_approval =
        PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
    mismatched_approval.phase = LifecyclePhase::Specification;
    mismatched_approval.revision = 2;
    mismatched_approval.domain_revision = 2;
    mismatched_approval.spec.current_candidate = Some(candidate.clone());
    mismatched_approval.spec.approval = Some(ApprovalRef {
        candidate_id: "cand_spec".to_string(),
        semantic_hash: "sha256:spec".to_string(),
        base_revision: 1,
        approval_event_seq: 1,
    });
    cases.push(("mismatched approval", mismatched_approval, true, false));

    let mut plan_stale =
        PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
    plan_stale.phase = LifecyclePhase::Planning;
    plan_stale.plan.current_candidate = Some(PlanCandidate {
        candidate_id: "cand_plan".to_string(),
        base_plan_revision: 1,
        plan_input_hash: "sha256:plan-input".to_string(),
        semantic_hash: "sha256:plan".to_string(),
        spec_candidate_id: "cand_spec".to_string(),
        spec_semantic_hash: "sha256:spec".to_string(),
        content: serde_json::json!({}),
        stale: true,
    });
    cases.push(("stale plan", plan_stale, false, true));

    for (label, state, expected_spec_stale, expected_plan_stale) in cases {
        let derived = state.derived();
        assert_eq!(derived.spec_stale, expected_spec_stale, "{label}");
        assert_eq!(derived.plan_stale, expected_plan_stale, "{label}");
    }

    let mut blocked =
        PlanningState::new("pln_1".to_string(), "prj_1".to_string(), "요청".to_string());
    blocked.phase = LifecyclePhase::Specification;
    blocked.spec.current_candidate = Some(candidate);
    blocked.blockers.insert(
        "blk_1".to_string(),
        Blocker {
            blocker_id: "blk_1".to_string(),
            created_event_seq: 1,
            created_ordinal: 0,
            revision: 1,
            kind: BlockerKind::Contradiction,
            severity: BlockerSeverity::Blocking,
            statement: "확인이 필요하다".to_string(),
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            resolved_at_revision: None,
        },
    );
    let derived = blocked.derived();
    assert!(derived.blocked);
    assert!(derived.waiting_for_spec_approval);
}
