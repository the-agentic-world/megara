use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::super::canonical::canonical_hash_with_aliases;
use super::super::domain::{EntityKind, EntityRevisionRef, PlanningState, VerificationMethod};
use super::error::CoreError;

pub(crate) fn validate_entity_refs(
    state: &PlanningState,
    refs: &[EntityRevisionRef],
) -> Result<(), CoreError> {
    let mut seen = BTreeSet::new();
    for reference in refs {
        if !seen.insert((&reference.id, reference.revision))
            || state
                .entities
                .at_revision(&reference.id, reference.revision)
                .is_none_or(|entity| !entity.is_current())
        {
            return Err(invalid(
                "candidate references must be unique current entity revisions",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_plan_content(
    state: &PlanningState,
    content: &Value,
) -> Result<(), CoreError> {
    let object = content
        .as_object()
        .ok_or_else(|| invalid("plan content must be an object"))?;
    exact_keys(
        object,
        &["baseline", "steps", "verifications", "plan_risks"],
    )?;
    let baseline = object
        .get("baseline")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("plan baseline is required"))?;
    exact_keys(baseline, &["commands", "known_failure_policy"])?;
    let commands = baseline
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("plan baseline commands are required"))?;
    if commands.is_empty()
        || commands
            .iter()
            .any(|command| command.as_str().is_none_or(|text| text.trim().is_empty()))
        || baseline
            .get("known_failure_policy")
            .and_then(Value::as_str)
            .is_none_or(|text| text.trim().is_empty())
    {
        return Err(invalid("plan baseline is incomplete"));
    }
    let steps = object
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("plan steps are required"))?;
    let mut step_ids = BTreeSet::new();
    let mut dependencies = BTreeMap::<String, Vec<String>>::new();
    for step in steps {
        let step = step
            .as_object()
            .ok_or_else(|| invalid("plan step must be an object"))?;
        exact_keys(
            step,
            &[
                "temp_ref",
                "objective",
                "requirement_refs",
                "depends_on",
                "change_surface",
                "risks",
                "rollback_or_recovery",
            ],
        )?;
        let temp_ref = nonblank(step, "temp_ref")?;
        if !step_ids.insert(temp_ref.to_string()) {
            return Err(invalid("plan step temp_ref must be unique"));
        }
        nonblank(step, "objective")?;
        nonblank(step, "rollback_or_recovery")?;
        let requirements = step
            .get("requirement_refs")
            .and_then(Value::as_array)
            .filter(|refs| !refs.is_empty())
            .ok_or_else(|| invalid("plan step requirement_refs are required"))?;
        let mut requirement_ids = BTreeSet::new();
        for reference in requirements {
            let reference: EntityRevisionRef = serde_json::from_value(reference.clone())
                .map_err(|_| invalid("plan requirement reference is malformed"))?;
            if !requirement_ids.insert((reference.id.clone(), reference.revision)) {
                return Err(invalid("plan requirement_refs must be unique"));
            }
            if state
                .entities
                .at_revision(&reference.id, reference.revision)
                .is_none_or(|entity| !entity.is_current() || entity.kind != EntityKind::Requirement)
            {
                return Err(invalid("plan requirement reference is invalid"));
            }
        }
        nonblank_array(step, "change_surface")?;
        let _risks = required_string_array(step, "risks")?;
        let deps = required_string_array(step, "depends_on")?;
        let mut dependency_ids = BTreeSet::new();
        if deps
            .iter()
            .any(|dependency| !dependency_ids.insert(*dependency))
        {
            return Err(invalid("plan depends_on references must be unique"));
        }
        dependencies.insert(
            temp_ref.to_string(),
            deps.iter()
                .map(|dependency| dependency.to_string())
                .collect(),
        );
    }
    for (step, deps) in &dependencies {
        if deps
            .iter()
            .any(|dependency| dependency == step || !step_ids.contains(dependency))
        {
            return Err(invalid(
                "plan dependency references an unknown or self step",
            ));
        }
    }
    if has_cycle(&dependencies) {
        return Err(invalid("plan dependencies must be acyclic"));
    }
    let mut covered_requirements = BTreeSet::new();
    for step in steps {
        if let Some(refs) = step.get("requirement_refs").and_then(Value::as_array) {
            for reference in refs {
                let reference: EntityRevisionRef = serde_json::from_value(reference.clone())
                    .map_err(|_| invalid("plan requirement reference is malformed"))?;
                covered_requirements.insert((reference.id, reference.revision));
            }
        }
    }
    if state
        .entities
        .current_requirements()
        .iter()
        .any(|entity| !covered_requirements.contains(&(entity.entity_id.clone(), entity.revision)))
    {
        return Err(invalid("every current requirement needs a plan step"));
    }
    validate_verifications(state, object, &step_ids)?;
    let risks = object
        .get("plan_risks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("plan_risks are required"))?;
    for risk in risks {
        let risk = risk
            .as_object()
            .ok_or_else(|| invalid("plan risk must be an object"))?;
        exact_keys(risk, &["statement", "mitigation"])?;
        nonblank(risk, "statement")?;
        nonblank(risk, "mitigation")?;
    }
    Ok(())
}

pub(crate) fn spec_semantic_hash(state: &PlanningState, content: &Value) -> String {
    let mut aliases = BTreeMap::new();
    for answer in &state.transcript.answers {
        aliases.insert(
            answer.answer_id.clone(),
            format!(
                "ANSWER@{}:{}",
                answer.created_event_seq, answer.created_ordinal
            ),
        );
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
    canonical_hash_with_aliases(content, Some(&aliases))
}

fn validate_verifications(
    state: &PlanningState,
    content: &serde_json::Map<String, Value>,
    step_ids: &BTreeSet<String>,
) -> Result<(), CoreError> {
    let verifications = content
        .get("verifications")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("plan verifications are required"))?;
    let mut verification_ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for verification in verifications {
        let verification = verification
            .as_object()
            .ok_or_else(|| invalid("verification must be an object"))?;
        exact_keys(
            verification,
            &[
                "temp_ref",
                "acceptance_criterion_ref",
                "plan_step_refs",
                "method",
                "procedure",
                "expected_result",
            ],
        )?;
        let temp_ref = nonblank(verification, "temp_ref")?;
        if !verification_ids.insert(temp_ref.to_string()) {
            return Err(invalid("verification temp_ref must be unique"));
        }
        let criterion: EntityRevisionRef = serde_json::from_value(
            verification
                .get("acceptance_criterion_ref")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .map_err(|_| invalid("verification acceptance criterion is malformed"))?;
        if state
            .entities
            .at_revision(&criterion.id, criterion.revision)
            .is_none_or(|entity| {
                !entity.is_current() || entity.kind != EntityKind::AcceptanceCriterion
            })
        {
            return Err(invalid("verification acceptance criterion is invalid"));
        }
        let refs = verification
            .get("plan_step_refs")
            .and_then(Value::as_array)
            .filter(|refs| !refs.is_empty())
            .ok_or_else(|| invalid("verification plan_step_refs are required"))?;
        if refs.iter().any(|reference| {
            reference
                .as_str()
                .is_none_or(|reference| !step_ids.contains(reference))
        }) {
            return Err(invalid("verification references an unknown plan step"));
        }
        let mut plan_step_refs = BTreeSet::new();
        for reference in refs {
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("verification plan_step_refs must be strings"))?;
            if !plan_step_refs.insert(reference) {
                return Err(invalid("verification plan_step_refs must be unique"));
            }
        }
        let _: VerificationMethod =
            serde_json::from_value(verification.get("method").cloned().unwrap_or(Value::Null))
                .map_err(|_| invalid("verification method is malformed"))?;
        nonblank(verification, "procedure")?;
        nonblank(verification, "expected_result")?;
        covered.insert((criterion.id, criterion.revision));
    }
    if state
        .entities
        .current_acceptance_criteria()
        .iter()
        .any(|entity| !covered.contains(&(entity.entity_id.clone(), entity.revision)))
    {
        return Err(invalid(
            "every current acceptance criterion needs a verification",
        ));
    }
    Ok(())
}

fn nonblank<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, CoreError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| invalid(format!("{key} must be nonblank")))
}

fn nonblank_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<&'a str>, CoreError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid(format!("{key} must not be empty")))?;
    if values
        .iter()
        .any(|value| value.as_str().is_none_or(|text| text.trim().is_empty()))
    {
        return Err(invalid(format!("{key} contains a blank value")));
    }
    Ok(values.iter().filter_map(Value::as_str).collect())
}

fn required_string_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<&'a str>, CoreError> {
    let values = object
        .get(key)
        .ok_or_else(|| invalid(format!("{key} must be an array")))?;
    let values = values
        .as_array()
        .ok_or_else(|| invalid(format!("{key} must be an array")))?;
    if values
        .iter()
        .any(|value| value.as_str().is_none_or(|text| text.trim().is_empty()))
    {
        return Err(invalid(format!("{key} contains a blank value")));
    }
    Ok(values.iter().filter_map(Value::as_str).collect())
}

fn exact_keys(object: &serde_json::Map<String, Value>, expected: &[&str]) -> Result<(), CoreError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid("plan content contains missing or unknown fields"));
    }
    Ok(())
}

fn has_cycle(graph: &BTreeMap<String, Vec<String>>) -> bool {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visiting.contains(node) {
            return true;
        }
        if !visited.insert(node.to_string()) {
            return false;
        }
        visiting.insert(node.to_string());
        let cycle = graph
            .get(node)
            .is_some_and(|deps| deps.iter().any(|dep| visit(dep, graph, visiting, visited)));
        visiting.remove(node);
        cycle
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    graph
        .keys()
        .any(|node| visit(node, graph, &mut visiting, &mut visited))
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::ProposalSchemaInvalid(message.into())
}
