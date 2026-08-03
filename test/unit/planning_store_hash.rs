use crate::planning::{
    domain::{
        Blocker, BlockerKind, BlockerSeverity, EntityBody, EntityDisposition, EntityKind,
        EntityRecord, EntityValidity, PendingQuestion, SourceRef,
    },
    store::normalized_state_hash,
};
use crate::planning_support::question;
use uuid::Uuid;

#[test]
fn normalized_hash_aliases_provenance_ids_map_keys_references_and_text_formatting() {
    let mut first = crate::planning_support::start_core().1;
    let mut second = first.clone();
    first.transcript.initial_request = "\n요청\r\n".to_string();
    second.transcript.initial_request = "요청  \n".to_string();
    let question_id_a = format!("qst_{}", Uuid::now_v7());
    let question_id_b = format!("qst_{}", Uuid::now_v7());
    let answer_id_a = format!("ans_{}", Uuid::now_v7());
    let answer_id_b = format!("ans_{}", Uuid::now_v7());
    first
        .transcript
        .answers
        .push(crate::planning::domain::AnswerRecord {
            answer_id: answer_id_a,
            created_event_seq: 5,
            created_ordinal: 0,
            question_id: question_id_a,
            based_on_revision: 4,
            text: "답\r\n  ".to_string(),
            selected_choice_ids: vec!["b".to_string(), "a".to_string()],
        });
    second
        .transcript
        .answers
        .push(crate::planning::domain::AnswerRecord {
            answer_id: answer_id_b,
            created_event_seq: 5,
            created_ordinal: 0,
            question_id: question_id_b,
            based_on_revision: 4,
            text: "답\n".to_string(),
            selected_choice_ids: vec!["a".to_string(), "b".to_string()],
        });
    let pending_id_a = format!("qst_{}", Uuid::now_v7());
    let pending_id_b = format!("qst_{}", Uuid::now_v7());
    let session_a = format!("pln_{}", Uuid::now_v7());
    let session_b = format!("pln_{}", Uuid::now_v7());
    first.session_id = session_a.clone();
    second.session_id = session_b.clone();
    first.required_model_action.as_mut().unwrap().session_id = session_a;
    second.required_model_action.as_mut().unwrap().session_id = session_b;
    first.required_model_action.as_mut().unwrap().work_item_id = format!("wrk_{}", Uuid::now_v7());
    second.required_model_action.as_mut().unwrap().work_item_id = format!("wrk_{}", Uuid::now_v7());
    let entity_uuid_a = Uuid::now_v7().to_string();
    let entity_uuid_b = Uuid::now_v7().to_string();
    let entity = |internal_uuid: String| EntityRecord {
        entity_id: "PROB-001".to_string(),
        internal_uuid,
        revision: 1,
        kind: EntityKind::Problem,
        body: EntityBody::Problem {
            statement: "문제".to_string(),
        },
        disposition: EntityDisposition::Current,
        validity: EntityValidity::Valid,
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        created_event_seq: 6,
        created_ordinal: 0,
    };
    first
        .entities
        .revisions
        .insert("PROB-001".to_string(), vec![entity(entity_uuid_a)]);
    second
        .entities
        .revisions
        .insert("PROB-001".to_string(), vec![entity(entity_uuid_b)]);
    let blocker_a = format!("blk_{}", Uuid::now_v7());
    let blocker_b = format!("blk_{}", Uuid::now_v7());
    first.blockers.insert(
        blocker_a.clone(),
        Blocker {
            blocker_id: blocker_a.clone(),
            created_event_seq: 7,
            created_ordinal: 1,
            revision: 1,
            kind: BlockerKind::EvidenceRequired,
            severity: BlockerSeverity::Blocking,
            statement: "근거\r\n".to_string(),
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            resolved_at_revision: None,
        },
    );
    second.blockers.insert(
        blocker_b.clone(),
        Blocker {
            blocker_id: blocker_b.clone(),
            created_event_seq: 7,
            created_ordinal: 1,
            revision: 1,
            kind: BlockerKind::EvidenceRequired,
            severity: BlockerSeverity::Blocking,
            statement: "근거\n".to_string(),
            source_refs: vec![SourceRef::InitialRequest {
                id: "request".to_string(),
            }],
            resolved_at_revision: None,
        },
    );
    first.required_model_action.as_mut().unwrap().context =
        serde_json::json!({"blocker_id": blocker_a, "note": "\r\n근거\t"});
    second.required_model_action.as_mut().unwrap().context =
        serde_json::json!({"blocker_id": blocker_b, "note": "근거"});
    assert_eq!(
        normalized_state_hash(&first),
        normalized_state_hash(&second)
    );

    let mut pending_first = first.clone();
    let mut pending_second = second.clone();
    pending_first.required_model_action = None;
    pending_second.required_model_action = None;
    pending_first.pending_question = Some(PendingQuestion {
        question_id: pending_id_a,
        created_event_seq: 9,
        created_ordinal: 0,
        based_on_revision: 9,
        proposal: question(),
    });
    pending_second.pending_question = Some(PendingQuestion {
        question_id: pending_id_b,
        created_event_seq: 9,
        created_ordinal: 0,
        based_on_revision: 9,
        proposal: question(),
    });
    assert_eq!(
        normalized_state_hash(&pending_first),
        normalized_state_hash(&pending_second)
    );
}
