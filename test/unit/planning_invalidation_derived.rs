use crate::planning::{domain::*, engine::*};
use crate::planning_support::{snapshot, start_core};

fn initial_source() -> Vec<SourceRef> {
    vec![SourceRef::InitialRequest {
        id: "request".to_string(),
    }]
}

fn audit_with_edges(state: &PlanningState) -> AuditCommand {
    let work_item = state.required_model_action.clone().unwrap();
    AuditCommand {
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
                temp_ref: "derived".to_string(),
                body: EntityBody::Constraint {
                    statement: "저장소에서 유도된 제약".to_string(),
                },
                source_refs: initial_source(),
            },
            EntityOp::Create {
                temp_ref: "dependent".to_string(),
                body: EntityBody::Risk {
                    statement: "유도된 제약에 의존하는 위험".to_string(),
                    impact: RiskImpact::Medium,
                    mitigation: "사용자에게 확인한다.".to_string(),
                },
                source_refs: initial_source(),
            },
            EntityOp::Create {
                temp_ref: "unrelated".to_string(),
                body: EntityBody::Problem {
                    statement: "독립 문제".to_string(),
                },
                source_refs: initial_source(),
            },
        ],
        edge_ops: vec![
            EdgeOp::Add {
                kind: EdgeKind::DerivedFrom,
                from: AuditEndpoint::TempRef {
                    temp_ref: "derived".to_string(),
                },
                to: AuditEndpoint::Source(SourceRef::Evidence {
                    id: "EVID-001".to_string(),
                }),
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            },
            EdgeOp::Add {
                kind: EdgeKind::DependsOn,
                from: AuditEndpoint::TempRef {
                    temp_ref: "dependent".to_string(),
                },
                to: AuditEndpoint::TempRef {
                    temp_ref: "derived".to_string(),
                },
                source_refs: initial_source(),
            },
        ],
        blocker_ops: Vec::new(),
        counterexample_review: None,
    }
}

#[test]
fn derived_from_evidence_then_depends_on_stales_reachable_entities_with_exact_causes() {
    let (mut core, started) = start_core();
    let refreshed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: started.session_id.clone(),
            expected_revision: started.revision,
            snapshot: snapshot("sha256:derived-before"),
        })
        .unwrap();
    let state = match refreshed {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("first evidence must change state"),
    };
    let applied = core.apply_audit(audit_with_edges(&state)).unwrap();
    let before = applied.state;
    let old_edges = before.entities.edges.clone();

    let refreshed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            snapshot: snapshot("sha256:derived-after"),
        })
        .unwrap();
    let result = match refreshed {
        EvidenceRefreshResult::Changed(result) => result,
        EvidenceRefreshResult::Unchanged { .. } => panic!("changed evidence must mutate state"),
    };
    for entity_id in ["CON-001", "RISK-001"] {
        let entity = result.state.entities.at_revision(entity_id, 1).unwrap();
        assert!(matches!(
            entity.validity,
            EntityValidity::Stale { ref causes, .. }
                if causes == &vec![SourceRef::Evidence { id: "EVID-001".to_string() }]
        ));
    }
    let work_item = result.state.required_model_action.as_ref().unwrap();
    let stale_entities = work_item.context["stale_entities"].as_array().unwrap();
    for entity_id in ["CON-001", "RISK-001"] {
        let stale = stale_entities
            .iter()
            .find(|entity| entity["entity_id"] == entity_id)
            .unwrap();
        assert_eq!(
            stale["validity"]["Stale"]["causes"][0],
            serde_json::json!({"kind":"evidence","id":"EVID-001"})
        );
    }
    assert!(result.state.entities.current("PROB-001").is_some());
    assert_eq!(result.state.entities.edges, old_edges);
}

#[test]
fn derived_from_unknown_source_is_rejected_without_state_or_event_change() {
    let (mut core, started) = start_core();
    let refreshed = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: started.session_id.clone(),
            expected_revision: started.revision,
            snapshot: snapshot("sha256:unknown-source"),
        })
        .unwrap();
    let state = match refreshed {
        EvidenceRefreshResult::Changed(result) => result.state,
        EvidenceRefreshResult::Unchanged { .. } => panic!("first evidence must change state"),
    };
    let mut command = audit_with_edges(&state);
    if let AuditEndpoint::Source(SourceRef::Evidence { id }) = command.edge_ops[0].to_endpoint_mut()
    {
        *id = "EVID-999".to_string();
    }
    let before = state.clone();
    let event_count = core.events().len();
    assert!(matches!(
        core.apply_audit(command),
        Err(CoreError::InvalidSourceReference)
    ));
    assert_eq!(core.state(&before.session_id), Some(&before));
    assert_eq!(core.events().len(), event_count);
}

trait EdgeEndpointMut {
    fn to_endpoint_mut(&mut self) -> &mut AuditEndpoint;
}

impl EdgeEndpointMut for EdgeOp {
    fn to_endpoint_mut(&mut self) -> &mut AuditEndpoint {
        match self {
            EdgeOp::Add { to, .. } => to,
            EdgeOp::Retire { .. } => panic!("test edge must be add"),
        }
    }
}
