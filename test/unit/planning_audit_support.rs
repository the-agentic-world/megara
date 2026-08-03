use crate::planning::{domain::*, engine::*};
use crate::planning_support::{required_entity_ops, snapshot, start_core};

pub(crate) fn with_full_audit_input(
    with_evidence: bool,
    missing_temp_ref: Option<&str>,
    with_blocker: bool,
) -> (InMemoryPlanningCore, PlanningState, ModelWorkItem) {
    let (mut core, mut state) = start_core();
    if with_evidence {
        state = match core
            .refresh_evidence(EvidenceRefreshCommand {
                session_id: state.session_id.clone(),
                expected_revision: state.revision,
                snapshot: snapshot("sha256:readiness-evidence"),
            })
            .unwrap()
        {
            EvidenceRefreshResult::Changed(result) => result.state,
            EvidenceRefreshResult::Unchanged { .. } => panic!("initial evidence must change"),
        };
    }
    let (mut entity_ops, edge_ops) = required_entity_ops();
    if let Some(missing) = missing_temp_ref {
        entity_ops.retain(|operation| {
            !matches!(operation, EntityOp::Create { temp_ref, .. } if temp_ref == missing)
        });
    }
    if !with_evidence {
        entity_ops.retain(|operation| {
            !matches!(operation, EntityOp::Create { temp_ref, .. } if temp_ref == "fact")
        });
    }
    let edge_ops = if missing_temp_ref == Some("edge") {
        Vec::new()
    } else if entity_ops.iter().any(
        |operation| matches!(operation, EntityOp::Create { temp_ref, .. } if temp_ref == "requirement"),
    ) && entity_ops.iter().any(
        |operation| matches!(operation, EntityOp::Create { temp_ref, .. } if temp_ref == "criterion"),
    ) {
        edge_ops
    } else {
        Vec::new()
    };
    let blocker_ops = if with_blocker {
        vec![BlockerOp::Create {
            temp_ref: "blocking".to_string(),
            kind: BlockerKind::ManualReviewRequired,
            severity: BlockerSeverity::Blocking,
            statement: "검토가 필요하다".to_string(),
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
        }]
    } else {
        Vec::new()
    };
    let work = state.required_model_action.clone().unwrap();
    let delta = core
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
            blocker_ops,
            counterexample_review: None,
        })
        .unwrap();
    let state = delta.state;
    let full_work = state.required_model_action.clone().unwrap();
    (core, state, full_work)
}
