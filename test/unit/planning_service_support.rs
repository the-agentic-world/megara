use crate::planning::domain::{AnswerMode, QuestionProposal, SourceRef};
use crate::planning::engine::{AuditCommand, AuditMode, AuditReadiness, EvidenceRefreshCommand};
use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::store::PlanningStore;
use crate::planning_store_support::snapshot;
use serde_json::json;

pub(crate) fn start_request(
    command_id: &str,
    request_id: &str,
    title: Option<&str>,
) -> LogicalRequest {
    LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: request_id.to_string(),
        operation: "planning.start".to_string(),
        command_id: Some(command_id.to_string()),
        session_id: None,
        expected_revision: None,
        params: Some(json!({"request":"서비스 계약을 검증한다.", "title":title})),
    }
}

pub(crate) fn question() -> QuestionProposal {
    QuestionProposal {
        context: "결정 배경입니다.".to_string(),
        question: "어떤 결과를 원하시나요?".to_string(),
        why_it_matters: "답에 따라 계획이 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Freeform {
            freeform_hint: "원하는 결과를 적어 주세요.".to_string(),
        },
    }
}

pub(crate) fn prepare_pending_question(store: &mut PlanningStore, session_id: &str) -> String {
    let state = store.current(session_id).unwrap();
    store
        .refresh_evidence(
            "cmd-evidence",
            "sha256:evidence",
            EvidenceRefreshCommand {
                session_id: session_id.to_string(),
                expected_revision: state.revision,
                snapshot: snapshot("sha256:evidence"),
            },
        )
        .unwrap();
    let state = store.current(session_id).unwrap();
    let work_item = state.required_model_action.clone().unwrap();
    store
        .apply_audit(
            "cmd-audit",
            "sha256:audit",
            AuditCommand {
                session_id: session_id.to_string(),
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
                counterexample_review: None,
            },
        )
        .unwrap();
    store
        .current(session_id)
        .unwrap()
        .pending_question
        .unwrap()
        .question_id
}
