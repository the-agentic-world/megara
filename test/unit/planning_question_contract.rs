use crate::planning::domain::{
    AnswerMode, Choice, QuestionAuthoring, QuestionProposal, Recommendation, SourceRef,
    TechnicalTerm,
};
use crate::planning::protocol::{project_question, QuestionProjectionBlock};
use crate::planning_support::question;
use serde_json::{json, Value};

fn choice_question() -> QuestionProposal {
    QuestionProposal {
        context: "배경을 설명합니다.".to_string(),
        question: "어느 방향을 선택할까요?".to_string(),
        why_it_matters: "선택에 따라 계획이 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Choice {
            choices: vec![
                Choice {
                    id: "keep".to_string(),
                    label: "유지".to_string(),
                    direction: "기존 방향을 유지합니다.".to_string(),
                    benefits: vec!["변경이 적습니다.".to_string()],
                    tradeoffs: vec!["기존 한계가 남습니다.".to_string()],
                },
                Choice {
                    id: "change".to_string(),
                    label: "바꾸기".to_string(),
                    direction: "새 방향으로 전환합니다.".to_string(),
                    benefits: vec!["새 요구를 반영합니다.".to_string()],
                    tradeoffs: vec!["확인할 내용이 늘어납니다.".to_string()],
                },
            ],
            recommendation: Some(Recommendation {
                choice_id: "keep".to_string(),
                reason: "현재 요청과 맞습니다.".to_string(),
                source_refs: vec![SourceRef::InitialRequest {
                    id: "request".to_string(),
                }],
            }),
            freeform_hint: "목록에 없으면 직접 설명해 주세요.".to_string(),
        },
    }
}

#[test]
fn question_authoring_gold_and_anti_examples_are_immutable_and_traceable() {
    let gold: Value = serde_json::from_str(include_str!(
        "../fixtures/planning/question_authoring/gold-v1.json"
    ))
    .unwrap();
    let proposal: QuestionProposal = serde_json::from_value(gold["question"].clone()).unwrap();
    assert!(proposal.validate_shape().is_ok());
    let projection = project_question(&proposal).unwrap();
    let kinds = projection
        .blocks
        .iter()
        .map(|block| match block {
            QuestionProjectionBlock::TechnicalTerm { .. } => "technical_term",
            QuestionProjectionBlock::Context { .. } => "context",
            QuestionProjectionBlock::Question { .. } => "question",
            QuestionProjectionBlock::WhyItMatters { .. } => "why_it_matters",
            QuestionProjectionBlock::Choice { .. } => "choice",
            QuestionProjectionBlock::Recommendation { .. } => "recommendation",
            QuestionProjectionBlock::FreeformHint { .. } => "freeform_hint",
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        gold["expected_blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .unwrap()
    );
    let authoring = QuestionAuthoring::v1();
    assert_eq!(authoring.version, "megara.question-authoring/v1");
    assert_eq!(
        authoring
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        gold["rubric_rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rule| rule.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        authoring
            .rules
            .iter()
            .map(|rule| rule.instruction.as_str())
            .collect::<Vec<_>>(),
        [
            "Megara와 구현 기술을 모르는 소프트웨어 기획 초심자를 독자로 둔다.",
            "쉬운 말 2~4문장으로 배경과 지금 결정할 이유를 설명한다.",
            "한 번에 하나의 결정만 묻는다.",
            "전문용어를 피하고, 불가피하면 뜻뿐 아니라 이 문맥의 역할과 중요성을 설명한다.",
            "각 선택지의 진행 방향, 장점, 감수할 점을 서로 겹치지 않게 설명한다.",
            "답에 따라 spec 또는 plan의 무엇이 달라지는지 설명한다.",
            "유효한 근거를 연결할 수 있을 때만 추천한다."
        ]
    );
    assert!(gold["source_material"][0]["text"]
        .as_str()
        .unwrap()
        .contains("기존 파일을 먼저 확인"));

    let anti_jargon: Value = serde_json::from_str(include_str!(
        "../fixtures/planning/question_authoring/anti-jargon.json"
    ))
    .unwrap();
    let anti_label: Value = serde_json::from_str(include_str!(
        "../fixtures/planning/question_authoring/anti-label-repeat.json"
    ))
    .unwrap();
    let jargon: QuestionProposal = serde_json::from_value(anti_jargon["question"].clone()).unwrap();
    let repeated: QuestionProposal =
        serde_json::from_value(anti_label["question"].clone()).unwrap();
    assert!(jargon.validate_shape().is_ok());
    assert!(repeated.validate_shape().is_err());
    assert!(anti_label["failure_rule_ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == "MPC-QST-004"));

    let rubric: Value = serde_json::from_str(include_str!(
        "../fixtures/planning/question_authoring/rubric-failure-map.json"
    ))
    .unwrap();
    let rows = rubric["rules"].as_array().unwrap();
    let gold_rows = rows
        .iter()
        .filter(|row| row["fixture"] == "gold-v1.json")
        .collect::<Vec<_>>();
    assert_eq!(gold_rows.len(), 7);
    assert!(gold_rows.iter().all(|row| row["decision"] == "yes"));
    for rule in [
        "audience",
        "context",
        "one-decision",
        "terms",
        "choices",
        "impact",
        "recommendation",
    ] {
        assert!(gold_rows.iter().any(|row| row["id"] == rule));
    }
    assert!(rows.iter().any(|row| row["fixture"] == "anti-jargon.json"
        && row["id"] == "terms"
        && row["decision"] == "no"));
    assert!(rows
        .iter()
        .any(|row| row["fixture"] == "anti-label-repeat.json"
            && row["id"] == "choices"
            && row["decision"] == "no"));
    let manual = include_str!("../fixtures/planning/question_authoring/manual-review-v1.md");
    for id in [
        "MPC-MAN-001",
        "MPC-MAN-002",
        "MPC-MAN-003",
        "MPC-MAN-004",
        "MPC-QST-015",
    ] {
        assert!(manual.contains(id));
    }
    assert!(manual.contains("signature:"));
    assert_eq!(
        projection.provenance.question_source_refs,
        proposal.source_refs
    );
}

#[test]
fn question_schema_negative_matrix_rejects_each_contract_violation() {
    let mut invalid = Vec::new();
    let mut blank_context = question();
    blank_context.context = " \n".to_string();
    invalid.push(blank_context);
    let mut blank_question = question();
    blank_question.question.clear();
    invalid.push(blank_question);
    let mut blank_why = question();
    blank_why.why_it_matters = "\t".to_string();
    invalid.push(blank_why);
    let mut zero = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut zero.answer {
        choices.clear();
    }
    invalid.push(zero);
    let mut one = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut one.answer {
        choices.truncate(1);
    }
    invalid.push(one);
    let mut duplicate = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut duplicate.answer {
        choices[1].id = choices[0].id.clone();
    }
    invalid.push(duplicate);
    let mut blank_id = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut blank_id.answer {
        choices[0].id = " ".to_string();
    }
    invalid.push(blank_id);
    let mut blank_label = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut blank_label.answer {
        choices[0].label.clear();
    }
    invalid.push(blank_label);
    let mut blank_direction = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut blank_direction.answer {
        choices[0].direction = "\t".to_string();
    }
    invalid.push(blank_direction);
    let mut empty_benefit = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut empty_benefit.answer {
        choices[0].benefits.clear();
    }
    invalid.push(empty_benefit);
    let mut blank_benefit = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut blank_benefit.answer {
        choices[0].benefits = vec![" ".to_string()];
    }
    invalid.push(blank_benefit);
    let mut empty_tradeoff = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut empty_tradeoff.answer {
        choices[0].tradeoffs.clear();
    }
    invalid.push(empty_tradeoff);
    let mut blank_tradeoff = choice_question();
    if let AnswerMode::Choice { choices, .. } = &mut blank_tradeoff.answer {
        choices[0].tradeoffs = vec!["\n".to_string()];
    }
    invalid.push(blank_tradeoff);
    let mut blank_hint = choice_question();
    if let AnswerMode::Choice { freeform_hint, .. } = &mut blank_hint.answer {
        *freeform_hint = "\n".to_string();
    }
    invalid.push(blank_hint);
    let mut partial = choice_question();
    if let AnswerMode::Choice { recommendation, .. } = &mut partial.answer {
        recommendation.as_mut().unwrap().reason.clear();
    }
    invalid.push(partial);
    let mut unknown_choice = choice_question();
    if let AnswerMode::Choice { recommendation, .. } = &mut unknown_choice.answer {
        recommendation.as_mut().unwrap().choice_id = "missing".to_string();
    }
    invalid.push(unknown_choice);
    let mut duplicate_terms = question();
    duplicate_terms.technical_terms = vec![
        TechnicalTerm {
            term: "용어".to_string(),
            plain_explanation: "설명".to_string(),
        },
        TechnicalTerm {
            term: "용어".to_string(),
            plain_explanation: "다른 설명".to_string(),
        },
    ];
    invalid.push(duplicate_terms);
    let mut blank_term = question();
    blank_term.technical_terms = vec![TechnicalTerm {
        term: "용어".to_string(),
        plain_explanation: String::new(),
    }];
    invalid.push(blank_term);
    for proposal in invalid {
        assert!(proposal.validate_shape().is_err());
    }

    let mut no_question_sources = question();
    no_question_sources.source_refs.clear();
    assert!(no_question_sources.validate_shape().is_err());
    let mut no_recommendation_sources = choice_question();
    if let AnswerMode::Choice { recommendation, .. } = &mut no_recommendation_sources.answer {
        recommendation.as_mut().unwrap().source_refs.clear();
    }
    assert!(no_recommendation_sources.validate_shape().is_err());
    let mut missing_recommendation = serde_json::to_value(choice_question()).unwrap();
    missing_recommendation["answer"]
        .as_object_mut()
        .unwrap()
        .remove("recommendation");
    assert!(serde_json::from_value::<QuestionProposal>(missing_recommendation).is_err());
    let mut missing_recommendation_reason = serde_json::to_value(choice_question()).unwrap();
    missing_recommendation_reason["answer"]["recommendation"]
        .as_object_mut()
        .unwrap()
        .remove("reason");
    assert!(serde_json::from_value::<QuestionProposal>(missing_recommendation_reason).is_err());
    let mut missing_recommendation_sources = serde_json::to_value(choice_question()).unwrap();
    missing_recommendation_sources["answer"]["recommendation"]
        .as_object_mut()
        .unwrap()
        .remove("source_refs");
    assert!(serde_json::from_value::<QuestionProposal>(missing_recommendation_sources).is_err());
    let mut freeform_with_choices = serde_json::to_value(question()).unwrap();
    freeform_with_choices["answer"]["choices"] = json!([]);
    assert!(serde_json::from_value::<QuestionProposal>(freeform_with_choices).is_err());
    let mut unknown_proposal = serde_json::to_value(choice_question()).unwrap();
    unknown_proposal["unknown"] = json!(true);
    assert!(serde_json::from_value::<QuestionProposal>(unknown_proposal).is_err());
    let mut unknown_choice_field = serde_json::to_value(choice_question()).unwrap();
    unknown_choice_field["answer"]["choices"][0]["unknown"] = json!(true);
    assert!(serde_json::from_value::<QuestionProposal>(unknown_choice_field).is_err());
    let mut unknown_recommendation_field = serde_json::to_value(choice_question()).unwrap();
    unknown_recommendation_field["answer"]["recommendation"]["unknown"] = json!(true);
    assert!(serde_json::from_value::<QuestionProposal>(unknown_recommendation_field).is_err());
    let mut missing_field = serde_json::to_value(question()).unwrap();
    missing_field.as_object_mut().unwrap().remove("context");
    assert!(serde_json::from_value::<QuestionProposal>(missing_field).is_err());
}
