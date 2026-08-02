use serde_json::json;

use super::*;

impl InMemoryPlanningCore {
    pub fn apply_audit(&mut self, command: AuditCommand) -> Result<MutationResult, CoreError> {
        self.mutate(
            &command.session_id,
            command.expected_revision,
            "planning.audit.apply",
            |state, effects| {
                if state.phase != LifecyclePhase::Interview {
                    return Err(CoreError::InvalidPhase(
                        "audit apply requires Interview".to_string(),
                    ));
                }
                validate_work_item(state, &command)?;
                if command.mode == AuditMode::Delta && command.readiness == AuditReadiness::Ready {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "delta audit cannot declare ready".to_string(),
                    ));
                }
                if command.mode == AuditMode::Full && !command.counterexample_review_performed {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "full audit requires counterexample review".to_string(),
                    ));
                }
                let domain_changed = apply_audit_ops(state, &command, effects)?;
                if command.mode == AuditMode::Delta {
                    match command.readiness {
                        AuditReadiness::Continue => {
                            let question = command.next_question.clone().ok_or_else(|| {
                                CoreError::ProposalSchemaInvalid(
                                    "delta continue requires one question".to_string(),
                                )
                            })?;
                            state.required_model_action = None;
                            state.pending_question = Some(PendingQuestion {
                                question_id: format!("qst_{}", state.revision + 1),
                                based_on_revision: state.revision + 1,
                                proposal: question,
                            });
                            effects.push(EventEffect::QuestionSet {
                                question_id: format!("qst_{}", state.revision + 1),
                            });
                        }
                        AuditReadiness::RequestFullAudit => {
                            if command.next_question.is_some() {
                                return Err(CoreError::ProposalSchemaInvalid(
                                    "full audit request cannot include a question".to_string(),
                                ));
                            }
                            state.required_model_action = Some(work_item(
                                state,
                                ModelActionKind::FullAudit,
                                command.input_hash.clone(),
                            ));
                            effects.push(EventEffect::ModelActionRequested {
                                kind: ModelActionKind::FullAudit,
                            });
                        }
                        AuditReadiness::Ready => unreachable!(),
                    }
                    return Ok(json!({"readiness": format!("{:?}", command.readiness)}));
                }

                let readiness_gate = compute_readiness_gate(
                    state,
                    &command.input_hash,
                    command.counterexample_review_performed,
                );
                if command.readiness == AuditReadiness::Ready && !readiness_gate.is_ready() {
                    return Err(CoreError::ProposalSchemaInvalid(
                        "core-computed readiness gate is incomplete".to_string(),
                    ));
                }
                if domain_changed || state.has_blocking_blocker() {
                    state.phase = LifecyclePhase::Interview;
                    state.pending_question = None;
                    state.full_audit = None;
                    if !domain_changed {
                        state.domain_revision += 1;
                    }
                    invalidate_artifacts(state, effects);
                    state.required_model_action = Some(work_item(
                        state,
                        ModelActionKind::DeltaAudit,
                        command.input_hash.clone(),
                    ));
                    effects.push(EventEffect::PhaseChanged {
                        phase: LifecyclePhase::Interview,
                    });
                    effects.push(EventEffect::ModelActionRequested {
                        kind: ModelActionKind::DeltaAudit,
                    });
                } else if command.readiness == AuditReadiness::Ready {
                    state.phase = LifecyclePhase::Specification;
                    state.pending_question = None;
                    state.required_model_action = Some(work_item(
                        state,
                        ModelActionKind::GenerateSpec,
                        command.input_hash.clone(),
                    ));
                    state.full_audit = Some(FullAuditRef {
                        input_hash: command.input_hash.clone(),
                        base_domain_revision: state.domain_revision,
                        counterexample_review_performed: true,
                    });
                    effects.push(EventEffect::PhaseChanged {
                        phase: LifecyclePhase::Specification,
                    });
                    effects.push(EventEffect::ModelActionRequested {
                        kind: ModelActionKind::GenerateSpec,
                    });
                } else {
                    let question = command.next_question.clone().ok_or_else(|| {
                        CoreError::ProposalSchemaInvalid(
                            "full continue without changes requires one question".to_string(),
                        )
                    })?;
                    state.required_model_action = None;
                    state.pending_question = Some(PendingQuestion {
                        question_id: format!("qst_{}", state.revision + 1),
                        based_on_revision: state.revision + 1,
                        proposal: question,
                    });
                    effects.push(EventEffect::QuestionSet {
                        question_id: format!("qst_{}", state.revision + 1),
                    });
                }
                Ok(json!({"readiness": format!("{:?}", command.readiness)}))
            },
        )
    }
}
