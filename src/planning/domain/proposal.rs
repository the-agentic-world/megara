// Proposal and question contracts.
use std::collections::BTreeSet;

use serde::{de::Deserializer, Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub type SessionId = String;
pub type ProjectId = String;
pub type QuestionId = String;
pub type WorkItemId = String;
pub type EntityId = String;
pub type BlockerId = String;
pub type CandidateId = String;
pub type EdgeId = String;

pub const QUESTION_AUTHORING_VERSION: &str = "megara.question-authoring/v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Interview,
    Specification,
    Planning,
    Complete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelActionKind {
    DeltaAudit,
    FullAudit,
    GenerateSpec,
    GeneratePlan,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringRule {
    pub id: String,
    pub instruction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionAuthoring {
    pub version: String,
    pub rules: Vec<AuthoringRule>,
}

impl QuestionAuthoring {
    pub fn v1() -> Self {
        Self {
            version: QUESTION_AUTHORING_VERSION.to_string(),
            rules: vec![
                AuthoringRule {
                    id: "audience".to_string(),
                    instruction:
                        "Megara와 구현 기술을 모르는 소프트웨어 기획 초심자를 독자로 둔다."
                            .to_string(),
                },
                AuthoringRule {
                    id: "context".to_string(),
                    instruction: "쉬운 말 2~4문장으로 배경과 지금 결정할 이유를 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "one-decision".to_string(),
                    instruction: "한 번에 하나의 결정만 묻는다.".to_string(),
                },
                AuthoringRule {
                    id: "terms".to_string(),
                    instruction: "전문용어를 피하고, 불가피하면 뜻뿐 아니라 이 문맥의 역할과 중요성을 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "choices".to_string(),
                    instruction: "각 선택지의 진행 방향, 장점, 감수할 점을 서로 겹치지 않게 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "impact".to_string(),
                    instruction: "답에 따라 spec 또는 plan의 무엇이 달라지는지 설명한다."
                        .to_string(),
                },
                AuthoringRule {
                    id: "recommendation".to_string(),
                    instruction: "유효한 근거를 연결할 수 있을 때만 추천한다.".to_string(),
                },
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TechnicalTerm {
    pub term: String,
    pub plain_explanation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub direction: String,
    pub benefits: Vec<String>,
    pub tradeoffs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recommendation {
    pub choice_id: String,
    pub reason: String,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnswerMode {
    Choice {
        choices: Vec<Choice>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        recommendation: Option<Recommendation>,
        freeform_hint: String,
    },
    Freeform {
        freeform_hint: String,
    },
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionProposal {
    pub context: String,
    pub question: String,
    pub why_it_matters: String,
    pub technical_terms: Vec<TechnicalTerm>,
    pub source_refs: Vec<SourceRef>,
    pub answer: AnswerMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterexampleResult {
    Resolved,
    Blocking,
    Advisory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterexampleFinding {
    pub statement: String,
    pub result: CounterexampleResult,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CounterexampleReview {
    pub performed: bool,
    pub challenged_entity_ids: Vec<EntityId>,
    pub findings: Vec<CounterexampleFinding>,
}

impl CounterexampleReview {
    pub fn performed() -> Self {
        Self {
            performed: true,
            challenged_entity_ids: Vec::new(),
            findings: Vec::new(),
        }
    }
}

impl QuestionProposal {
    pub fn validate_shape(&self) -> Result<(), String> {
        let non_empty = |value: &str| !value.trim().is_empty();
        if !non_empty(&self.context)
            || !non_empty(&self.question)
            || !non_empty(&self.why_it_matters)
        {
            return Err("context, question, and why_it_matters must not be blank".to_string());
        }
        if self.source_refs.is_empty() {
            return Err("question source_refs must not be empty".to_string());
        }
        let mut terms = BTreeSet::new();
        for term in &self.technical_terms {
            if !non_empty(&term.term) || !non_empty(&term.plain_explanation) {
                return Err("technical terms require a term and plain explanation".to_string());
            }
            let normalized = term.term.trim().nfc().collect::<String>();
            if !terms.insert(normalized) {
                return Err("technical terms must be unique after NFC normalization".to_string());
            }
        }
        match &self.answer {
            AnswerMode::Freeform { freeform_hint } => {
                if !non_empty(freeform_hint) {
                    return Err("freeform_hint must not be blank".to_string());
                }
            }
            AnswerMode::Choice {
                choices,
                recommendation,
                freeform_hint,
            } => {
                if choices.len() < 2 || !non_empty(freeform_hint) {
                    return Err(
                        "choice mode requires at least two choices and a freeform hint".to_string(),
                    );
                }
                let mut choice_ids = BTreeSet::new();
                for choice in choices {
                    if !non_empty(&choice.id)
                        || !non_empty(&choice.label)
                        || !non_empty(&choice.direction)
                        || choice.benefits.is_empty()
                        || choice.tradeoffs.is_empty()
                        || choice.benefits.iter().any(|item| !non_empty(item))
                        || choice.tradeoffs.iter().any(|item| !non_empty(item))
                        || !choice_ids.insert(choice.id.clone())
                    {
                        return Err(
                            "choices must have unique IDs and complete explanations".to_string()
                        );
                    }
                }
                if let Some(recommendation) = recommendation {
                    if !choice_ids.contains(&recommendation.choice_id)
                        || !non_empty(&recommendation.reason)
                        || recommendation.source_refs.is_empty()
                    {
                        return Err(
                            "recommendation must reference a choice with reason and sources"
                                .to_string(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceRef {
    InitialRequest {
        id: String,
    },
    Answer {
        id: String,
    },
    Evidence {
        id: String,
    },
    Entity {
        id: EntityId,
        revision: u64,
    },
    ApprovedSpec {
        id: CandidateId,
        semantic_hash: String,
    },
}
