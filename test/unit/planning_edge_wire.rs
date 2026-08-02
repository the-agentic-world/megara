use crate::planning::domain::{CounterexampleReview, EdgeKind, SourceRef};
use crate::planning::engine::{
    AuditCommand, AuditEndpoint, AuditMode, AuditReadiness, EdgeOp, EvidenceRefreshCommand,
};
use crate::planning_support::{required_entity_ops, snapshot, start_core};
use serde_json::json;

#[test]
fn edge_add_retire_and_source_endpoint_use_exact_v1_wire_shapes() {
    let add = EdgeOp::Add {
        kind: EdgeKind::DerivedFrom,
        from: AuditEndpoint::TempRef {
            temp_ref: "tmp_fact".to_string(),
        },
        to: AuditEndpoint::Source(SourceRef::Evidence {
            id: "EVID-001".to_string(),
        }),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
    };
    assert_eq!(
        serde_json::to_value(add).unwrap(),
        json!({
            "op":"add",
            "kind":"derived_from",
            "from":{"temp_ref":"tmp_fact"},
            "to":{"kind":"evidence","id":"EVID-001"},
            "source_refs":[{"kind":"initial_request","id":"request"}]
        })
    );

    let entity_target = AuditEndpoint::Entity {
        entity_id: "AC-001".to_string(),
        revision: 1,
    };
    assert_eq!(
        serde_json::to_value(entity_target).unwrap(),
        json!({"entity_id":"AC-001","revision":1})
    );

    let retire = EdgeOp::Retire {
        edge_id: "edge_3_0".to_string(),
        base_edge_revision: 1,
        reason: "교체된 요구사항".to_string(),
    };
    assert_eq!(
        serde_json::to_value(retire).unwrap(),
        json!({
            "op":"retire",
            "edge_id":"edge_3_0",
            "base_edge_revision":1,
            "reason":"교체된 요구사항"
        })
    );
}

#[test]
fn evidence_snapshot_requires_explicit_evidence_array() {
    let value = json!({
        "evidence_hash":"sha256:evidence",
        "head_oid":null,
        "head_ref":null,
        "dirty":false,
        "status_hash":"sha256:status",
        "cited_files_hash":"sha256:files"
    });
    assert!(
        serde_json::from_value::<crate::planning::domain::RepoEvidenceSnapshot>(value).is_err()
    );
}

#[test]
fn edge_retire_applies_and_revision_mismatch_is_atomic() {
    let (mut core, started) = start_core();
    let evidence = core
        .refresh_evidence(EvidenceRefreshCommand {
            session_id: started.session_id.clone(),
            expected_revision: started.revision,
            snapshot: snapshot("sha256:edge-retire"),
        })
        .unwrap();
    let state = match evidence {
        crate::planning::engine::EvidenceRefreshResult::Changed(result) => result.state,
        crate::planning::engine::EvidenceRefreshResult::Unchanged { .. } => {
            panic!("evidence must change")
        }
    };
    let work = state.required_model_action.clone().unwrap();
    let (entity_ops, edge_ops) = required_entity_ops();
    let added = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops,
            edge_ops,
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    let full = added.state.required_model_action.clone().unwrap();
    let edge_id = added.state.entities.edges[0].edge_id.clone();
    let retired = core
        .apply_audit(AuditCommand {
            session_id: added.state.session_id.clone(),
            expected_revision: added.state.revision,
            work_item_id: full.work_item_id,
            mode: AuditMode::Full,
            base_revision: full.base_revision,
            base_domain_revision: full.base_domain_revision,
            input_hash: full.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: vec![EdgeOp::Retire {
                edge_id: edge_id.clone(),
                base_edge_revision: 1,
                reason: "요구사항 revision 교체".to_string(),
            }],
            blocker_ops: Vec::new(),
            counterexample_review: Some(CounterexampleReview::performed()),
        })
        .unwrap();
    assert!(retired.state.entities.edges[0].retired);
    assert_eq!(retired.state.entities.edges[0].revision, 2);

    let before = retired.state.clone();
    let event_count = core.events().len();
    let next = before.required_model_action.clone().unwrap();
    assert!(matches!(
        core.apply_audit(AuditCommand {
            session_id: before.session_id.clone(),
            expected_revision: before.revision,
            work_item_id: next.work_item_id,
            mode: AuditMode::Delta,
            base_revision: next.base_revision,
            base_domain_revision: next.base_domain_revision,
            input_hash: next.input_hash,
            readiness: AuditReadiness::RequestFullAudit,
            next_question: None,
            entity_ops: Vec::new(),
            edge_ops: vec![EdgeOp::Retire {
                edge_id,
                base_edge_revision: 1,
                reason: "두 번 retire 금지".to_string(),
            }],
            blocker_ops: Vec::new(),
            counterexample_review: None,
        }),
        Err(crate::planning::engine::CoreError::ProposalBaseMismatch)
    ));
    assert_eq!(core.state(&before.session_id), Some(&before));
    assert_eq!(core.events().len(), event_count);
}
