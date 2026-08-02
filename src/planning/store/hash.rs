use std::collections::BTreeMap;

use super::super::domain::PlanningState;

pub fn normalized_state_hash(state: &PlanningState) -> String {
    let aliases = generated_aliases(state);
    super::super::canonical::canonical_hash_with_aliases(state, Some(&aliases))
}

fn generated_aliases(state: &PlanningState) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    aliases.insert(state.session_id.clone(), "SESSION@1:0".to_string());
    if let Some(work_item) = &state.required_model_action {
        aliases.insert(
            work_item.work_item_id.clone(),
            format!(
                "WORK_ITEM@{}:{}",
                work_item.created_event_seq, work_item.created_ordinal
            ),
        );
    }

    if let Some(question) = &state.pending_question {
        aliases.insert(
            question.question_id.clone(),
            format!(
                "QUESTION@{}:{}",
                question.created_event_seq, question.created_ordinal
            ),
        );
    }
    for answer in &state.transcript.answers {
        aliases
            .entry(answer.question_id.clone())
            .or_insert_with(|| format!("QUESTION@{}:0", answer.based_on_revision));
        aliases.insert(
            answer.answer_id.clone(),
            format!(
                "ANSWER@{}:{}",
                answer.created_event_seq, answer.created_ordinal
            ),
        );
    }
    for (map_id, blocker) in &state.blockers {
        let alias = format!(
            "BLOCKER@{}:{}",
            blocker.created_event_seq, blocker.created_ordinal
        );
        aliases.insert(map_id.clone(), alias.clone());
        aliases.insert(blocker.blocker_id.clone(), alias);
    }
    for records in state.entities.revisions.values() {
        for record in records {
            aliases.insert(
                record.internal_uuid.clone(),
                format!(
                    "ENTITY@{}:{}",
                    record.created_event_seq, record.created_ordinal
                ),
            );
        }
    }
    aliases
}
