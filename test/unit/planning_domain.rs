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
