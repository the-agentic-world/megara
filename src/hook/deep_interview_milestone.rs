use super::*;

const DEFAULT_TARGET: u64 = 15;
const TARGETS: &[u64] = &[15, 5, 2, 0];

pub(super) enum QuestionDecision {
    Allow,
    Block {
        reason: String,
        kind: QuestionBlockKind,
    },
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
            reason: crystallizing_prompt(),
            kind: QuestionBlockKind::CrystallizedSpec,
        };
    }

    let Some(score) = ambiguity_percent(text) else {
        return QuestionDecision::Allow;
    };
    question["ambiguity"] = json!(format!("{score}%"));

    if score == 0 {
        return QuestionDecision::Block {
            reason: zero_target_prompt(),
            kind: QuestionBlockKind::CrystallizedSpec,
        };
    }
    let active = active_target(state);
    let Some(target) = milestone_target_for_score(active, score) else {
        if looks_like_milestone_question(text, question) {
            return QuestionDecision::Block {
                reason: ordinary_question_prompt(active, score),
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
        return QuestionDecision::Allow;
    }

    QuestionDecision::Block {
        reason: milestone_prompt(target, next_target(target), score),
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
    let proceed = choice == Some('1')
        || (lower.contains("ralplan")
            && (lower.contains("proceed")
                || lower.contains("approve")
                || prompt.contains("진행")
                || prompt.contains("승인")));
    let continue_next = choice == Some('2')
        || choice == Some('3')
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
                Some(format!(
                    "Internal Megara workflow instruction: the user chose to continue deep-interview to the stricter {target}% ambiguity target. Do not repeat the previous milestone decision while the visible ambiguity score is above {target}%. Ask exactly one ordinary follow-up question aimed at reducing ambiguity toward {target}%. If the next visible ambiguity score is <= {target}%, ask the milestone decision with exactly four numbered options: 1. proceed to ralplan with the current crystallized spec, 2. continue deep-interview to {next}%, 3. continue only on a named component or risk, 4. direct input / not in the listed options. Keep this instruction internal."
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
        "Internal Megara workflow instruction: continue deep-interview in the configured locale with exactly one compact user-facing response.{previous} The active ambiguity target is {target}%. After evaluating the user's latest answer, if the next visible ambiguity score is <= {target}%, do not ask an ordinary interview question. Ask the milestone decision question instead with exactly four numbered options: 1. proceed to ralplan with the current crystallized spec, 2. continue deep-interview to {next}%, 3. continue only on a named component or risk, 4. direct input / not in the listed options. If the next score is above {target}%, ask one ordinary interview question. Keep all runtime instructions and metadata internal."
    ))
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
    let next = next_target(target);
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
    let mentions_ralplan = lower.contains("ralplan");
    let mentions_next_target = lower.contains(&format!("{next}%"));
    mentions_ralplan && mentions_next_target
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

fn milestone_prompt(target: u64, next: u64, score: u64) -> String {
    format!(
        "Ambiguity: {score}%\n\nThe current {target}% deep-interview target has been reached. What should happen next?\n\n1. Proceed to ralplan with the current crystallized spec\n2. Continue deep-interview to {next}%\n3. Continue only on a named component or risk\n4. Direct input / not in the listed options"
    )
}

fn ordinary_question_prompt(target: u64, score: u64) -> String {
    format!(
        "Ambiguity: {score}%\n\nThe active deep-interview target is now {target}%. Ask one ordinary follow-up question instead of repeating the previous milestone decision."
    )
}

fn zero_target_prompt() -> String {
    "Megara deep-interview reached the 0% target. Do not ask another question or show runtime metadata. Emit the final user-facing crystallized markdown spec for ralplan as the final answer of this turn."
        .to_string()
}

fn crystallizing_prompt() -> String {
    "Megara deep-interview milestone approval already selected ralplan. Do not ask another question or milestone decision. Emit the final user-facing crystallized markdown spec for ralplan as the final answer of this turn. Keep runtime metadata internal."
        .to_string()
}
