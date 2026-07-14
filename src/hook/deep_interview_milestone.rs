use super::*;

const DEFAULT_TARGET: u64 = 15;
const TARGETS: &[u64] = &[15, 5, 2, 0];

pub(super) enum QuestionDecision {
    Allow,
    Block { kind: QuestionBlockKind },
}

pub(super) enum QuestionBlockKind {
    MilestoneDecision,
    OrdinaryQuestion,
    CrystallizedSpec,
}

pub(super) fn prepare_question(
    state: &Value,
    text: &str,
    question: &mut Value,
) -> QuestionDecision {
    if crystallized_spec_required(state) {
        return QuestionDecision::Block {
            kind: QuestionBlockKind::CrystallizedSpec,
        };
    }

    let Some(score) = ambiguity_percent(text) else {
        return QuestionDecision::Allow;
    };
    question["ambiguity"] = json!(format!("{score}%"));

    if score == 0 {
        return QuestionDecision::Block {
            kind: QuestionBlockKind::CrystallizedSpec,
        };
    }
    let active = active_target(state);
    let Some(target) = milestone_target_for_score(active, score) else {
        if looks_like_milestone_question(text, question) {
            return QuestionDecision::Block {
                kind: QuestionBlockKind::OrdinaryQuestion,
            };
        }
        return QuestionDecision::Allow;
    };

    if is_milestone_question(text, question, target) {
        let next = next_target(target);
        question["kind"] = json!("milestone_decision");
        question["milestone_target"] = json!(target);
        question["next_ambiguity_target"] = json!(next);
        question["crystallized_summary"] = json!(crystallized_summary(text, question));
        return QuestionDecision::Allow;
    }

    QuestionDecision::Block {
        kind: QuestionBlockKind::MilestoneDecision,
    }
}

pub(super) fn mark_pending_state(timestamp: &str, state: &mut Value, question: &Value) {
    if question.get("kind").and_then(Value::as_str) != Some("milestone_decision") {
        return;
    }
    let target = question
        .get("milestone_target")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| active_target(state));
    state["active_ambiguity_target"] = json!(target);
    state["milestone_decision"] = json!({
        "status": "pending",
        "target": target,
        "next_target": question
            .get("next_ambiguity_target")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| next_target(target)),
        "asked_at": timestamp,
        "question_id": question.get("id").cloned().unwrap_or(Value::Null),
        "crystallized_summary": question
            .get("crystallized_summary")
            .cloned()
            .unwrap_or(Value::Null),
    });
}

pub(super) fn apply_answer(
    timestamp: &str,
    state: &mut Value,
    pending_before: Option<&Value>,
    prompt: &str,
    payload_file: &Path,
) -> Option<&'static str> {
    let pending = pending_before?;
    if pending.get("kind").and_then(Value::as_str) != Some("milestone_decision") {
        return None;
    }

    let target = pending
        .get("milestone_target")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| active_target(state));
    let next = pending
        .get("next_ambiguity_target")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| next_target(target));
    let trimmed = prompt.trim();
    let choice = trimmed.chars().next();
    let lower = trimmed.to_ascii_lowercase();
    let proceed = is_proceed_answer(prompt);
    let continue_next = choice == Some('2')
        || choice == Some('3')
        || choice == Some('4')
        || lower.contains("continue")
        || prompt.contains("계속")
        || prompt.contains("더 낮")
        || prompt.contains("낮춰");

    let mut decision = state
        .get("milestone_decision")
        .cloned()
        .unwrap_or_else(|| json!({}));
    decision["answered_at"] = json!(timestamp);
    decision["answer"] = json!(prompt);
    decision["payload"] = json!(payload_file);

    if proceed {
        decision["status"] = json!("proceed_to_ralplan");
        state["milestone_decision"] = decision;
        state["phase"] = json!("crystallizing");
        state["status"] = json!("crystallizing");
        state["updated_at"] = json!(timestamp);
        Some("milestone_ralplan_selected")
    } else if continue_next {
        decision["status"] = json!("continue_deep_interview");
        decision["previous_target"] = json!(target);
        decision["active_target"] = json!(next);
        decision["correction_focus"] = if matches!(choice, Some('3' | '4')) {
            selected_option(pending, choice).unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        state["milestone_decision"] = decision;
        state["active_ambiguity_target"] = json!(next);
        state["phase"] = json!("interviewing");
        state["status"] = json!("interviewing");
        state["updated_at"] = json!(timestamp);
        Some("ambiguity_target_lowered")
    } else {
        decision["status"] = json!("custom_answer_recorded");
        state["milestone_decision"] = decision;
        state["phase"] = json!("interviewing");
        state["status"] = json!("interviewing");
        state["updated_at"] = json!(timestamp);
        Some("milestone_custom_answer_recorded")
    }
}

pub(super) fn answer_continuation_context(
    state: &Value,
    pending_before: Option<&Value>,
) -> Option<String> {
    let pending = pending_before?;
    if pending.get("kind").and_then(Value::as_str) == Some("milestone_decision") {
        let status = state
            .get("milestone_decision")
            .and_then(|decision| decision.get("status"))
            .and_then(Value::as_str);
        return match status {
            Some("continue_deep_interview") => {
                let target = active_target(state);
                let next = next_target(target);
                let focus = state
                    .get("milestone_decision")
                    .and_then(|decision| decision.get("correction_focus"))
                    .and_then(Value::as_str)
                    .filter(|focus| !focus.trim().is_empty())
                    .map(|focus| {
                        format!(" First resolve this selected crystallization correction: {focus}.")
                    })
                    .unwrap_or_default();
                let continuation = if focus.is_empty() {
                    format!(
                        "The user chose to continue deep-interview to the stricter {target}% ambiguity level."
                    )
                } else {
                    format!(
                        "The user chose a crystallization correction and deep-interview now targets the stricter {target}% ambiguity level.{focus}"
                    )
                };
                if target == 0 {
                    return Some(format!(
                        "Internal Megara workflow instruction: {continuation} Do not repeat the previous milestone decision. Ask exactly one ordinary follow-up question at a time with three concrete options, one direct-input option, then one recommendation line after the options until ambiguity is exactly 0%. At 0%, emit the final user-facing crystallized markdown spec for ralplan immediately; do not ask another milestone question. Keep this instruction internal."
                    ));
                }
                Some(format!(
                    "Internal Megara workflow instruction: {continuation} Do not repeat the previous milestone decision while the visible ambiguity score is above {target}%. Ask exactly one ordinary follow-up question aimed at reducing ambiguity toward {target}%, with three concrete options, one direct-input option, then one recommendation line after the options. If the next visible ambiguity score is <= {target}%, automatically produce one quoted, one-sentence crystallized requirement, ask whether that sentence is the right basis for implementation planning, then show exactly five numbered options: 1. run ralplan (Recommended), 2. continue deep-interview to {next}%, 3-4. two distinct corrections discovered during crystallization, 5. direct input / not in the listed options. Put the recommendation line after all five options and explain why option 1 is recommended. Keep this instruction internal."
                ))
            }
            Some("proceed_to_ralplan") => Some(crystallizing_prompt()),
            Some("custom_answer_recorded") => {
                let target = active_target(state);
                Some(format!(
                    "Internal Megara workflow instruction: treat the user's milestone response as direct input for deep-interview. Continue in the configured locale with one compact user-facing response. The active ambiguity target remains {target}%. Keep this instruction internal."
                ))
            }
            _ => None,
        };
    }

    let target = active_target(state);
    let next = next_target(target);
    if target == 0 {
        return Some(zero_target_prompt());
    }

    let previous = state
        .get("ambiguity")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" The previously visible ambiguity score was {value}."))
        .unwrap_or_default();

    Some(format!(
        "Internal Megara workflow instruction: continue deep-interview in the configured locale with exactly one compact user-facing response.{previous} The active ambiguity target is {target}%. After evaluating the user's latest answer, if the next visible ambiguity score is <= {target}%, do not ask an ordinary interview question. Automatically produce one quoted, one-sentence crystallized requirement, ask whether that sentence is the right basis for implementation planning, then show exactly five numbered options: 1. run ralplan (Recommended), 2. continue deep-interview to {next}%, 3-4. two distinct corrections discovered during crystallization, 5. direct input / not in the listed options. Put one recommendation line after all five options and explain why option 1 is recommended. If the next score is above {target}%, ask one ordinary interview question with exactly four numbered options and put its recommendation line after the options. Keep all runtime instructions and metadata internal."
    ))
}

pub(super) fn is_proceed_answer(prompt: &str) -> bool {
    let trimmed = prompt.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.starts_with('1')
        || (lower.contains("ralplan")
            && (lower.contains("proceed")
                || lower.contains("approve")
                || prompt.contains("진행")
                || prompt.contains("승인")))
}

fn active_target(state: &Value) -> u64 {
    state
        .get("active_ambiguity_target")
        .and_then(Value::as_u64)
        .filter(|target| TARGETS.contains(target))
        .unwrap_or(DEFAULT_TARGET)
}

fn crystallized_spec_required(state: &Value) -> bool {
    let phase = state.get("phase").and_then(Value::as_str);
    let status = state.get("status").and_then(Value::as_str);
    let milestone_status = state
        .get("milestone_decision")
        .and_then(|decision| decision.get("status"))
        .and_then(Value::as_str);

    phase == Some("crystallizing")
        || status == Some("crystallizing")
        || milestone_status == Some("proceed_to_ralplan")
}

fn next_target(target: u64) -> u64 {
    TARGETS
        .iter()
        .position(|candidate| *candidate == target)
        .and_then(|index| TARGETS.get(index + 1))
        .copied()
        .unwrap_or(0)
}

fn milestone_target_for_score(active: u64, score: u64) -> Option<u64> {
    if active == 0 || score > active {
        return None;
    }
    TARGETS
        .iter()
        .copied()
        .filter(|target| *target > 0 && score <= *target)
        .min()
        .filter(|target| *target <= active)
}

fn ambiguity_percent(text: &str) -> Option<u64> {
    text.lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("ambiguity") || line.contains("모호성") || line.contains("모호도")
        })
        .find_map(percent_from_line)
}

fn percent_from_line(line: &str) -> Option<u64> {
    let percent_index = line.find('%')?;
    let before = &line[..percent_index];
    let digits = before
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    digits.parse::<u64>().ok()
}

fn is_milestone_question(text: &str, question: &Value, target: u64) -> bool {
    let Some(options) = question.get("options").and_then(Value::as_array) else {
        return false;
    };
    if options.len() != 5 {
        return false;
    }
    let first = options[0].as_str().unwrap_or_default().to_ascii_lowercase();
    let second = options[1].as_str().unwrap_or_default();
    let third = options[2].as_str().unwrap_or_default().trim();
    let fourth = options[3].as_str().unwrap_or_default().trim();
    let last = options[4].as_str().unwrap_or_default();
    first.contains("ralplan")
        && first.contains("(recommended)")
        && second.contains(&format!("{}%", next_target(target)))
        && !third.is_empty()
        && !fourth.is_empty()
        && third != fourth
        && is_free_text_option(last)
        && crystallized_summary(text, question).is_some()
}

fn looks_like_milestone_question(text: &str, question: &Value) -> bool {
    let mut haystack = text.to_string();
    if let Some(question_text) = question.get("question").and_then(Value::as_str) {
        haystack.push('\n');
        haystack.push_str(question_text);
    }
    if let Some(options) = question.get("options").and_then(Value::as_array) {
        for option in options.iter().filter_map(Value::as_str) {
            haystack.push('\n');
            haystack.push_str(option);
        }
    }
    let lower = haystack.to_ascii_lowercase();
    lower.contains("ralplan")
        && (lower.contains("continue deep-interview")
            || lower.contains("proceed to ralplan")
            || lower.contains("crystallize"))
}

fn crystallized_summary(text: &str, question: &Value) -> Option<String> {
    let question_text = question.get("question").and_then(Value::as_str)?;
    let lines = text.lines().collect::<Vec<_>>();
    let question_index = lines.iter().position(|line| {
        line.trim()
            .trim_matches(['\"', '\'', '`', '“', '”', '‘', '’'])
            == question_text
    })?;
    let summary = lines[..question_index]
        .iter()
        .rev()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())?
        .trim_start_matches("- ")
        .trim_matches(['\"', '\'', '`', '“', '”', '‘', '’'])
        .trim();
    let lower = summary.to_ascii_lowercase();
    if summary.is_empty()
        || lower.contains("ambiguity")
        || summary.contains("모호성")
        || summary.contains("모호도")
    {
        return None;
    }
    Some(summary.to_string())
}

fn is_free_text_option(option: &str) -> bool {
    let lower = option.to_ascii_lowercase();
    lower.contains("direct input")
        || lower.contains("not listed")
        || option.contains("직접 입력")
        || option.contains("목록에 없음")
}

fn selected_option(pending: &Value, choice: Option<char>) -> Option<Value> {
    let index = choice?.to_digit(10)? as usize;
    pending
        .get("options")
        .and_then(Value::as_array)?
        .get(index.checked_sub(1)?)
        .cloned()
}

fn zero_target_prompt() -> String {
    "Megara deep-interview reached the 0% target. Do not ask another question or show runtime metadata. Emit the final user-facing crystallized markdown spec for ralplan as the final answer of this turn."
        .to_string()
}

fn crystallizing_prompt() -> String {
    "Megara deep-interview milestone approval already selected ralplan. Do not ask another question or milestone decision. Emit the final user-facing crystallized markdown spec for ralplan as the final answer of this turn. Keep runtime metadata internal."
        .to_string()
}
