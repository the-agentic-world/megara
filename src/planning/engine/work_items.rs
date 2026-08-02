use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::super::domain::*;

pub(crate) fn work_item(state: &PlanningState, kind: ModelActionKind) -> ModelWorkItem {
    let output_schema = match kind {
        ModelActionKind::DeltaAudit | ModelActionKind::FullAudit => "megara.audit-proposal/v1",
        ModelActionKind::GenerateSpec => "megara.spec-proposal/v1",
        ModelActionKind::GeneratePlan => "megara.plan-proposal/v1",
    };
    let question_authoring = matches!(
        kind,
        ModelActionKind::DeltaAudit | ModelActionKind::FullAudit
    )
    .then(QuestionAuthoring::v1);
    let context = work_item_context(state, kind, question_authoring.as_ref());
    let base_revision = state.revision + 1;
    let base_domain_revision = state.domain_revision;
    let base_plan_revision = state.plan_revision;
    let aliases = work_item_aliases(state);
    let input_hash = if kind == ModelActionKind::GeneratePlan {
        plan_input_hash(state)
    } else {
        super::super::canonical::canonical_hash_with_aliases(&context, Some(&aliases))
    };
    let work_item_basis = BTreeMap::from([
        ("base_domain_revision", json!(base_domain_revision)),
        ("base_plan_revision", json!(base_plan_revision)),
        ("base_revision", json!(base_revision)),
        ("input_hash", json!(input_hash)),
        ("kind", json!(kind)),
        ("output_schema", json!(output_schema)),
        ("session_id", json!(state.session_id)),
    ]);
    let work_item_hash = super::super::canonical::canonical_hash(&work_item_basis);
    let work_item_id = format!("wrk_{}", work_item_hash.trim_start_matches("sha256:"));
    ModelWorkItem {
        kind,
        work_item_id,
        created_event_seq: base_revision,
        created_ordinal: 0,
        session_id: state.session_id.clone(),
        base_revision,
        base_domain_revision,
        base_plan_revision,
        input_hash,
        output_schema: output_schema.to_string(),
        context,
    }
}

pub(crate) fn plan_input_hash(state: &PlanningState) -> String {
    let mut aliases = BTreeMap::new();
    if let (Some(candidate), Some(approval)) = (
        state.spec.current_candidate.as_ref(),
        state.spec.approval.as_ref(),
    ) {
        if candidate.candidate_id == approval.candidate_id {
            aliases.insert(
                approval.candidate_id.clone(),
                format!(
                    "SPEC_CANDIDATE@{}:{}",
                    candidate.created_event_seq, candidate.created_ordinal
                ),
            );
        }
    }
    let basis = json!({
        "schema": "megara.plan-proposal/v1",
        "approved_spec": state.spec.approval.as_ref().map(|approval| json!({
            "candidate_id": approval.candidate_id,
            "semantic_hash": approval.semantic_hash,
        })),
        "evidence_hash": state.repo_snapshot.as_ref().map(|snapshot| snapshot.evidence_hash.clone()),
        "plan_revision": state.plan_revision,
    });
    super::super::canonical::canonical_hash_with_aliases(&basis, Some(&aliases))
}

fn work_item_context(
    state: &PlanningState,
    kind: ModelActionKind,
    question_authoring: Option<&QuestionAuthoring>,
) -> Value {
    let current_entities = state
        .entities
        .revisions
        .values()
        .filter_map(|records| records.iter().rev().find(|record| record.is_current()))
        .map(entity_record_value)
        .collect::<Vec<_>>();
    let stale_entities = state
        .entities
        .revisions
        .values()
        .filter_map(|records| records.iter().max_by_key(|record| record.revision))
        .filter(|record| matches!(record.validity, EntityValidity::Stale { .. }))
        .map(entity_record_value)
        .collect::<Vec<_>>();
    let mut current_edges = state
        .entities
        .edges
        .iter()
        .filter(|edge| !edge.retired)
        .collect::<Vec<_>>();
    current_edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let blockers = state.blockers.values().collect::<Vec<_>>();
    let repo_snapshot = state.repo_snapshot.as_ref().map(|snapshot| {
        let mut value =
            serde_json::to_value(snapshot).expect("snapshot serialization is infallible");
        strip_capture_metadata(&mut value);
        value
    });
    let mut context = json!({
        "initial_request": state.transcript.initial_request,
        "current_entities": current_entities,
        "stale_entities": stale_entities,
        "current_edges": current_edges,
        "blockers": blockers,
        "repo_snapshot": repo_snapshot,
    });
    if let Some(question_authoring) = question_authoring {
        context["question_authoring"] = json!(question_authoring);
    }
    if kind == ModelActionKind::GenerateSpec {
        context["full_audit"] = json!(state.full_audit);
    }
    if kind == ModelActionKind::GeneratePlan {
        context["approved_spec"] = json!({
            "candidate": state.spec.current_candidate,
            "approval": state.spec.approval,
        });
        context["plan_revision"] = json!(state.plan_revision);
    }
    if kind == ModelActionKind::FullAudit {
        context["transcript"] = json!(state.transcript);
    } else {
        context["latest_answer"] = json!(state.transcript.answers.last());
    }
    context
}

fn work_item_aliases(state: &PlanningState) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    aliases.insert(state.session_id.clone(), "SESSION@1:0".to_string());
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
        aliases.insert(
            answer.answer_id.clone(),
            format!(
                "ANSWER@{}:{}",
                answer.created_event_seq, answer.created_ordinal
            ),
        );
        aliases
            .entry(answer.question_id.clone())
            .or_insert_with(|| format!("QUESTION@{}:0", answer.based_on_revision));
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
    if let Some(candidate) = &state.spec.current_candidate {
        aliases.insert(
            candidate.candidate_id.clone(),
            format!(
                "SPEC_CANDIDATE@{}:{}",
                candidate.created_event_seq, candidate.created_ordinal
            ),
        );
    }
    if let Some(candidate) = &state.plan.current_candidate {
        aliases.insert(
            candidate.candidate_id.clone(),
            format!(
                "PLAN_CANDIDATE@{}:{}",
                candidate.created_event_seq, candidate.created_ordinal
            ),
        );
    }
    aliases
}

fn strip_capture_metadata(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("captured_at");
            for child in object.values_mut() {
                strip_capture_metadata(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_capture_metadata(child);
            }
        }
        _ => {}
    }
}
