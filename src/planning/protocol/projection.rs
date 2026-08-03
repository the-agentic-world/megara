use serde::{Deserialize, Serialize};

use super::super::domain::{QuestionProposal, SourceRef};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuestionProjectionBlock {
    TechnicalTerm {
        term: String,
        plain_explanation: String,
    },
    Context {
        text: String,
    },
    Question {
        text: String,
    },
    WhyItMatters {
        text: String,
    },
    Choice {
        id: String,
        label: String,
        direction: String,
        benefits: Vec<String>,
        tradeoffs: Vec<String>,
    },
    Recommendation {
        choice_id: String,
        reason: String,
        source_refs: Vec<SourceRef>,
    },
    FreeformHint {
        text: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionProjectionProvenance {
    pub question_source_refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionProjection {
    pub blocks: Vec<QuestionProjectionBlock>,
    pub provenance: QuestionProjectionProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionError(pub String);

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProjectionError {}

pub fn project_question(
    proposal: &QuestionProposal,
) -> Result<QuestionProjection, ProjectionError> {
    proposal.validate_shape().map_err(ProjectionError)?;
    let mut blocks = Vec::new();
    for term in &proposal.technical_terms {
        blocks.push(QuestionProjectionBlock::TechnicalTerm {
            term: term.term.clone(),
            plain_explanation: term.plain_explanation.clone(),
        });
    }
    blocks.push(QuestionProjectionBlock::Context {
        text: proposal.context.clone(),
    });
    blocks.push(QuestionProjectionBlock::Question {
        text: proposal.question.clone(),
    });
    blocks.push(QuestionProjectionBlock::WhyItMatters {
        text: proposal.why_it_matters.clone(),
    });
    match &proposal.answer {
        super::super::domain::AnswerMode::Choice {
            choices,
            recommendation,
            freeform_hint,
        } => {
            for choice in choices {
                blocks.push(QuestionProjectionBlock::Choice {
                    id: choice.id.clone(),
                    label: choice.label.clone(),
                    direction: choice.direction.clone(),
                    benefits: choice.benefits.clone(),
                    tradeoffs: choice.tradeoffs.clone(),
                });
            }
            if let Some(recommendation) = recommendation {
                blocks.push(QuestionProjectionBlock::Recommendation {
                    choice_id: recommendation.choice_id.clone(),
                    reason: recommendation.reason.clone(),
                    source_refs: recommendation.source_refs.clone(),
                });
            }
            blocks.push(QuestionProjectionBlock::FreeformHint {
                text: freeform_hint.clone(),
            });
        }
        super::super::domain::AnswerMode::Freeform { freeform_hint } => {
            blocks.push(QuestionProjectionBlock::FreeformHint {
                text: freeform_hint.clone(),
            });
        }
    }
    Ok(QuestionProjection {
        blocks,
        provenance: QuestionProjectionProvenance {
            question_source_refs: proposal.source_refs.clone(),
        },
    })
}
