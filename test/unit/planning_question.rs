use crate::planning::domain::{AnswerMode, Choice, QuestionProposal, SourceRef, TechnicalTerm};
use crate::planning::protocol::{project_question, QuestionProjectionBlock};
use crate::planning_support::question;

#[test]
fn question_shape_accepts_valid_freeform_and_choice_variants() {
    assert!(question().validate_shape().is_ok());
    let choice = QuestionProposal {
        context: "배경을 설명합니다.".to_string(),
        question: "한 가지를 선택해 주세요.".to_string(),
        why_it_matters: "선택에 따라 명세가 달라집니다.".to_string(),
        technical_terms: vec![TechnicalTerm {
            term: "기록".to_string(),
            plain_explanation: "나중에 다시 확인할 수 있게 남기는 정보".to_string(),
        }],
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Choice {
            choices: vec![
                Choice {
                    id: "keep".to_string(),
                    label: "남겨두기".to_string(),
                    direction: "기록을 참고자료로 보존합니다.".to_string(),
                    benefits: vec!["기존 정보를 잃지 않습니다.".to_string()],
                    tradeoffs: vec!["확인할 내용이 늘어납니다.".to_string()],
                },
                Choice {
                    id: "fresh".to_string(),
                    label: "새로 시작하기".to_string(),
                    direction: "현재 요청만으로 기획합니다.".to_string(),
                    benefits: vec!["상태가 단순합니다.".to_string()],
                    tradeoffs: vec!["예전 결정을 다시 물어야 합니다.".to_string()],
                },
            ],
            recommendation: None,
            freeform_hint: "원하는 방향을 직접 설명해도 됩니다.".to_string(),
        },
    };
    assert!(choice.validate_shape().is_ok());
}

#[test]
fn question_projection_preserves_order_provenance_and_exact_occurrences() {
    let proposal = QuestionProposal {
        context: "배경 sentinel".to_string(),
        question: "질문 sentinel".to_string(),
        why_it_matters: "영향 sentinel".to_string(),
        technical_terms: vec![TechnicalTerm {
            term: "용어 sentinel".to_string(),
            plain_explanation: "쉽게 설명 sentinel".to_string(),
        }],
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Choice {
            choices: vec![
                Choice {
                    id: "choice-a".to_string(),
                    label: "첫째 sentinel".to_string(),
                    direction: "첫 방향 sentinel".to_string(),
                    benefits: vec!["장점 sentinel".to_string()],
                    tradeoffs: vec!["대가 sentinel".to_string()],
                },
                Choice {
                    id: "choice-b".to_string(),
                    label: "둘째 sentinel".to_string(),
                    direction: "둘 방향 sentinel".to_string(),
                    benefits: vec!["둘째 편익 sentinel".to_string()],
                    tradeoffs: vec!["둘째 부담 sentinel".to_string()],
                },
            ],
            recommendation: Some(crate::planning::domain::Recommendation {
                choice_id: "choice-a".to_string(),
                reason: "추천 sentinel".to_string(),
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }),
            freeform_hint: "직접입력 sentinel".to_string(),
        },
    };
    let projection = project_question(&proposal).unwrap();
    let expected = [
        "technical",
        "context",
        "question",
        "why",
        "choice",
        "choice",
        "recommendation",
        "freeform",
    ];
    let actual = projection
        .blocks
        .iter()
        .map(|block| match block {
            QuestionProjectionBlock::TechnicalTerm { .. } => "technical",
            QuestionProjectionBlock::Context { .. } => "context",
            QuestionProjectionBlock::Question { .. } => "question",
            QuestionProjectionBlock::WhyItMatters { .. } => "why",
            QuestionProjectionBlock::Choice { .. } => "choice",
            QuestionProjectionBlock::Recommendation { .. } => "recommendation",
            QuestionProjectionBlock::FreeformHint { .. } => "freeform",
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    for value in [
        "용어 sentinel",
        "쉽게 설명 sentinel",
        "배경 sentinel",
        "질문 sentinel",
        "영향 sentinel",
        "첫째 sentinel",
        "첫 방향 sentinel",
        "장점 sentinel",
        "대가 sentinel",
        "둘째 sentinel",
        "둘 방향 sentinel",
        "둘째 편익 sentinel",
        "둘째 부담 sentinel",
        "추천 sentinel",
        "직접입력 sentinel",
    ] {
        let encoded = serde_json::to_string(&projection).unwrap();
        assert_eq!(encoded.matches(value).count(), 1, "{value}");
    }
    assert_eq!(
        projection.provenance.question_source_refs,
        proposal.source_refs
    );
    assert!(matches!(
        projection.blocks[4],
        QuestionProjectionBlock::Choice { ref id, .. } if id == "choice-a"
    ));
    assert!(matches!(
        projection.blocks[5],
        QuestionProjectionBlock::Choice { ref id, .. } if id == "choice-b"
    ));
    assert!(matches!(
        projection.blocks[6],
        QuestionProjectionBlock::Recommendation { ref choice_id, .. } if choice_id == "choice-a"
    ));
}

#[test]
fn freeform_projection_has_no_choice_or_recommendation_blocks() {
    let projection = project_question(&question()).unwrap();
    assert!(projection.blocks.iter().all(|block| {
        !matches!(
            block,
            QuestionProjectionBlock::Choice { .. } | QuestionProjectionBlock::Recommendation { .. }
        )
    }));
}
