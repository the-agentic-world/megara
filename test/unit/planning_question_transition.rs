use crate::planning::domain::{AnswerMode, Choice, QuestionProposal, TechnicalTerm};
use crate::planning::engine::{AuditCommand, AuditMode, AuditReadiness, CoreError};
use crate::planning_support::{question, start_core};

#[test]
fn invalid_question_proposals_are_atomic_and_keep_work_item() {
    let invalid = [
        QuestionProposal {
            context: String::new(),
            ..question()
        },
        QuestionProposal {
            technical_terms: vec![
                TechnicalTerm {
                    term: "용어".to_string(),
                    plain_explanation: "설명".to_string(),
                },
                TechnicalTerm {
                    term: "용어".to_string(),
                    plain_explanation: "다른 설명".to_string(),
                },
            ],
            ..question()
        },
        QuestionProposal {
            answer: AnswerMode::Choice {
                choices: vec![Choice {
                    id: "only".to_string(),
                    label: "하나".to_string(),
                    direction: "방향".to_string(),
                    benefits: vec!["장점".to_string()],
                    tradeoffs: vec!["감수할 점".to_string()],
                }],
                recommendation: None,
                freeform_hint: "직접 답해도 됩니다.".to_string(),
            },
            ..question()
        },
    ];
    for proposal in invalid {
        let (mut core, state) = start_core();
        let work = state.required_model_action.clone().unwrap();
        let before = core.clone();
        let result = core.apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(proposal),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        });
        assert!(matches!(result, Err(CoreError::ProposalSchemaInvalid(_))));
        assert_eq!(
            core.sessions().collect::<Vec<_>>(),
            before.sessions().collect::<Vec<_>>()
        );
        assert_eq!(core.events(), before.events());
    }
}

#[test]
fn invalid_question_can_be_corrected_with_same_work_item_and_new_submission() {
    let (mut core, state) = start_core();
    let work = state.required_model_action.clone().unwrap();
    let invalid = core.apply_audit(AuditCommand {
        session_id: state.session_id.clone(),
        expected_revision: state.revision,
        work_item_id: work.work_item_id.clone(),
        mode: AuditMode::Delta,
        base_revision: work.base_revision,
        base_domain_revision: work.base_domain_revision,
        input_hash: work.input_hash.clone(),
        readiness: AuditReadiness::Continue,
        next_question: Some(QuestionProposal {
            context: String::new(),
            ..question()
        }),
        entity_ops: Vec::new(),
        edge_ops: Vec::new(),
        blocker_ops: Vec::new(),
        counterexample_review: None,
    });
    assert!(matches!(invalid, Err(CoreError::ProposalSchemaInvalid(_))));
    assert_eq!(core.events().len(), 1);
    let corrected = core
        .apply_audit(AuditCommand {
            session_id: state.session_id.clone(),
            expected_revision: state.revision,
            work_item_id: work.work_item_id,
            mode: AuditMode::Delta,
            base_revision: work.base_revision,
            base_domain_revision: work.base_domain_revision,
            input_hash: work.input_hash,
            readiness: AuditReadiness::Continue,
            next_question: Some(question()),
            entity_ops: Vec::new(),
            edge_ops: Vec::new(),
            blocker_ops: Vec::new(),
            counterexample_review: None,
        })
        .unwrap();
    assert_eq!(corrected.state.revision, 2);
    assert!(corrected.state.pending_question.is_some());
    assert_eq!(core.events().len(), 2);
}
