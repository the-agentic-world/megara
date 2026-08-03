use std::collections::{BTreeMap, BTreeSet};

use super::readiness_validation::*;
use super::*;
use uuid::Uuid;

pub(crate) fn apply_audit_ops(
    state: &mut PlanningState,
    command: &AuditCommand,
    effects: &mut Vec<EventEffect>,
) -> Result<bool, CoreError> {
    let has_ops = !command.entity_ops.is_empty()
        || !command.edge_ops.is_empty()
        || !command.blocker_ops.is_empty();
    if !has_ops {
        return Ok(false);
    }

    let event_seq = state.revision + 1;
    let mut temp_refs = BTreeMap::<String, EntityRef>::new();
    let mut all_temp_refs = BTreeSet::new();
    let mut entity_ordinal = 0_u32;
    for operation in &command.entity_ops {
        match operation {
            EntityOp::Create {
                temp_ref,
                body,
                source_refs,
            } => {
                if temp_ref.trim().is_empty() || !all_temp_refs.insert(temp_ref.clone()) {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "entity temp_ref must be unique and non-empty".to_string(),
                    ));
                }
                let kind = body_kind(body);
                validate_entity_body(body)?;
                validate_entity_sources(state, kind, source_refs)?;
                validate_fact_evidence_refs(state, body, source_refs)?;
                let entity_id = next_entity_id(&state.entities, kind);
                let internal_uuid = Uuid::now_v7().to_string();
                let entity_ref = EntityRef {
                    id: entity_id.clone(),
                    revision: 1,
                };
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id: entity_id.clone(),
                        internal_uuid: internal_uuid.clone(),
                        revision: 1,
                        kind,
                        body: body.clone(),
                        disposition: EntityDisposition::Current,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityCreated {
                    entity_id,
                    revision: 1,
                    internal_uuid,
                });
                temp_refs.insert(temp_ref.clone(), entity_ref);
                entity_ordinal += 1;
            }
            EntityOp::Revise {
                entity_id,
                base_entity_revision,
                body,
                source_refs,
            } => {
                let previous = state
                    .entities
                    .revisions
                    .get(entity_id)
                    .and_then(|records| records.iter().max_by_key(|record| record.revision))
                    .cloned()
                    .ok_or_else(|| {
                        CoreError::ProposalSchemaInvalid(
                            "revised entity must have a recorded revision".to_string(),
                        )
                    })?;
                if previous.disposition != EntityDisposition::Current {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "only the latest current entity revision can be revised".to_string(),
                    ));
                }
                if previous.revision != *base_entity_revision || body_kind(body) != previous.kind {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_body(body)?;
                validate_entity_sources(state, previous.kind, source_refs)?;
                validate_fact_evidence_refs(state, body, source_refs)?;
                if let Some(records) = state.entities.revisions.get_mut(entity_id) {
                    if let Some(previous) = records
                        .iter_mut()
                        .find(|record| record.revision == *base_entity_revision)
                    {
                        previous.disposition = EntityDisposition::Superseded;
                    }
                }
                let next_revision = *base_entity_revision + 1;
                let internal_uuid = Uuid::now_v7().to_string();
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id: entity_id.clone(),
                        internal_uuid: internal_uuid.clone(),
                        revision: next_revision,
                        kind: previous.kind,
                        body: body.clone(),
                        disposition: EntityDisposition::Current,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityCreated {
                    entity_id: entity_id.clone(),
                    revision: next_revision,
                    internal_uuid,
                });
                state
                    .entities
                    .add_edge(supersedes_edge(
                        entity_id,
                        next_revision,
                        *base_entity_revision,
                        source_refs.clone(),
                        event_seq,
                    ))
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityInvalidated {
                    entity_id: entity_id.clone(),
                });
                entity_ordinal += 1;
            }
            EntityOp::Reject {
                entity_id,
                base_entity_revision,
                reason,
                source_refs,
            } => {
                if reason.trim().is_empty() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "rejection reason must not be blank".to_string(),
                    ));
                }
                let current = state.entities.current(entity_id).cloned().ok_or_else(|| {
                    CoreError::ProposalSchemaInvalid("rejected entity must be current".to_string())
                })?;
                if current.revision != *base_entity_revision {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                validate_entity_sources(state, current.kind, source_refs)?;
                if let Some(records) = state.entities.revisions.get_mut(entity_id) {
                    if let Some(previous) = records
                        .iter_mut()
                        .find(|record| record.revision == *base_entity_revision)
                    {
                        previous.disposition = EntityDisposition::Superseded;
                    }
                }
                let next_revision = *base_entity_revision + 1;
                let internal_uuid = Uuid::now_v7().to_string();
                state
                    .entities
                    .insert(EntityRecord {
                        entity_id: entity_id.clone(),
                        internal_uuid: internal_uuid.clone(),
                        revision: next_revision,
                        kind: current.kind,
                        body: current.body,
                        disposition: EntityDisposition::Rejected,
                        validity: EntityValidity::Valid,
                        source_refs: source_refs.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: entity_ordinal,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
                effects.push(EventEffect::EntityCreated {
                    entity_id: entity_id.clone(),
                    revision: next_revision,
                    internal_uuid,
                });
                effects.push(EventEffect::EntityInvalidated {
                    entity_id: entity_id.clone(),
                });
                entity_ordinal += 1;
            }
        }
    }

    for (edge_ordinal, operation) in command.edge_ops.iter().enumerate() {
        match operation {
            EdgeOp::Add {
                kind,
                from,
                to,
                source_refs,
            } => {
                validate_source_refs_exist(state, source_refs)?;
                let from = resolve_entity_endpoint(from, &temp_refs)?;
                let to = match resolve_endpoint(to, &temp_refs)? {
                    AuditEndpoint::Entity {
                        entity_id,
                        revision,
                    } => EdgeTarget::Entity(EntityRef {
                        id: entity_id,
                        revision,
                    }),
                    AuditEndpoint::Source(source_ref) => {
                        validate_source_refs_exist(state, std::slice::from_ref(&source_ref))?;
                        EdgeTarget::Source(source_ref)
                    }
                    AuditEndpoint::TempRef { .. } => unreachable!(),
                };
                state
                    .entities
                    .add_edge(Edge {
                        edge_id: format!("edge_{event_seq}_{edge_ordinal}"),
                        revision: 1,
                        kind: *kind,
                        from,
                        to,
                        source_refs: source_refs.clone(),
                        retired: false,
                    })
                    .map_err(CoreError::ProposalSchemaInvalid)?;
            }
            EdgeOp::Retire {
                edge_id,
                base_edge_revision,
                reason,
            } => {
                if edge_id.trim().is_empty() || reason.trim().is_empty() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "edge retirement requires edge_id and reason".to_string(),
                    ));
                }
                let edge = state
                    .entities
                    .edges
                    .iter_mut()
                    .find(|edge| edge.edge_id == *edge_id)
                    .ok_or_else(|| {
                        CoreError::ProposalSchemaInvalid("edge to retire was not found".to_string())
                    })?;
                if edge.retired || edge.revision != *base_edge_revision {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                edge.retired = true;
                edge.revision += 1;
            }
        }
    }

    for (blocker_ordinal, operation) in command.blocker_ops.iter().enumerate() {
        match operation {
            BlockerOp::Create {
                temp_ref,
                kind,
                severity,
                statement,
                source_refs,
            } => {
                if temp_ref.trim().is_empty()
                    || statement.trim().is_empty()
                    || source_refs.is_empty()
                    || !all_temp_refs.insert(temp_ref.clone())
                {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "blocker fields must not be blank".to_string(),
                    ));
                }
                validate_source_refs_exist(state, source_refs)?;
                let blocker_id = format!("blk_{}", Uuid::now_v7());
                state.blockers.insert(
                    blocker_id.clone(),
                    Blocker {
                        blocker_id: blocker_id.clone(),
                        created_event_seq: event_seq,
                        created_ordinal: blocker_ordinal as u32,
                        revision: 1,
                        kind: *kind,
                        severity: *severity,
                        statement: statement.clone(),
                        source_refs: source_refs.clone(),
                        resolved_at_revision: None,
                    },
                );
                effects.push(EventEffect::BlockerCreated { blocker_id });
            }
            BlockerOp::Resolve {
                blocker_id,
                base_blocker_revision,
                resolution,
                source_refs,
            } => {
                if resolution.trim().is_empty() || source_refs.is_empty() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "blocker resolution fields must not be blank".to_string(),
                    ));
                }
                validate_source_refs_exist(state, source_refs)?;
                let blocker = state.blockers.get_mut(blocker_id).ok_or_else(|| {
                    CoreError::ProposalSchemaInvalid("blocker not found".to_string())
                })?;
                if blocker.revision != *base_blocker_revision
                    || blocker.resolved_at_revision.is_some()
                {
                    return Err(CoreError::ProposalBaseMismatch);
                }
                blocker.revision += 1;
                blocker.resolved_at_revision = Some(event_seq);
                blocker.statement = resolution.clone();
                blocker.source_refs = source_refs.clone();
            }
        }
    }
    state.domain_revision += 1;
    Ok(true)
}
