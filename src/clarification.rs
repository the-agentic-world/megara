use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarificationRequest {
    #[serde(rename = "type")]
    pub request_type: String,
    pub blocking: bool,
    pub summary: String,
    pub questions: Vec<ClarificationQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarificationQuestion {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
}

impl ClarificationRequest {
    pub fn is_valid_request(&self) -> bool {
        self.request_type == "clarification_request" && self.blocking && !self.questions.is_empty()
    }
}

pub fn parse_clarification_request(text: &str) -> Result<Option<ClarificationRequest>> {
    if let Ok(request) = serde_json::from_str::<ClarificationRequest>(text) {
        return Ok(request.is_valid_request().then_some(request));
    }

    for candidate in clarification_json_candidates(text) {
        let Ok(request) = serde_json::from_str::<ClarificationRequest>(candidate) else {
            continue;
        };
        if request.is_valid_request() {
            return Ok(Some(request));
        }
    }

    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(request) = find_clarification_in_value(&value)? {
            return Ok(Some(request));
        }
    }

    Ok(None)
}

pub fn format_clarification_comment(
    run_id: &str,
    clarification_id: &str,
    request: &ClarificationRequest,
) -> String {
    let questions = request
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let mut rendered = format!("{}. {}", index + 1, question.question);
            if !question.options.is_empty() {
                for option in &question.options {
                    rendered.push_str(&format!("\n   - {option}"));
                }
            }
            rendered
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"Sisyphus needs clarification before starting this task.

**Summary**
{summary}

**Questions**
{questions}

Reply to this comment or update the issue description. Sisyphus will retry after new context is detected.

<!-- sisyphus:run={run_id}; clarification={clarification_id} -->
"#,
        summary = request.summary.as_str(),
        questions = questions,
        run_id = run_id,
        clarification_id = clarification_id
    )
}

fn clarification_json_candidates(text: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut search_start = 0;

    while let Some(relative_index) = text[search_start..].find("\"type\"") {
        let type_index = search_start + relative_index;
        let Some(object_start) = text[..type_index].rfind('{') else {
            search_start = type_index + 6;
            continue;
        };
        let Some(object_end) = find_matching_brace(text, object_start) else {
            search_start = type_index + 6;
            continue;
        };
        let candidate = &text[object_start..=object_end];
        if candidate.contains("clarification_request") {
            candidates.push(candidate);
        }
        search_start = object_end + 1;
    }

    candidates
}

fn find_matching_brace(text: &str, object_start: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in text[object_start..].bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(object_start + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn find_clarification_in_value(value: &Value) -> Result<Option<ClarificationRequest>> {
    match value {
        Value::String(text) => parse_clarification_request(text),
        Value::Array(values) => {
            for nested in values {
                if let Some(request) = find_clarification_in_value(nested)? {
                    return Ok(Some(request));
                }
            }
            Ok(None)
        }
        Value::Object(map) => {
            if map
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|value| value == "clarification_request")
            {
                let request: ClarificationRequest = serde_json::from_value(value.clone())
                    .context("failed to parse clarification request object")?;
                return Ok(request.is_valid_request().then_some(request));
            }

            for nested in map.values() {
                if let Some(request) = find_clarification_in_value(nested)? {
                    return Ok(Some(request));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_clarification_json() {
        let parsed = parse_clarification_request(
            r#"{
              "type": "clarification_request",
              "blocking": true,
              "summary": "Missing target flow.",
              "questions": [
                {
                  "id": "target_flow",
                  "question": "Which flow should change?",
                  "options": ["email", "oauth"]
                }
              ]
            }"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed.summary, "Missing target flow.");
        assert_eq!(parsed.questions[0].id, "target_flow");
    }

    #[test]
    fn parses_clarification_json_inside_jsonl_message() {
        let jsonl = r#"{"type":"message","content":"{\"type\":\"clarification_request\",\"blocking\":true,\"summary\":\"Need scope.\",\"questions\":[{\"id\":\"scope\",\"question\":\"What scope?\",\"options\":[\"A\",\"B\"]}]}"}"#;
        let parsed = parse_clarification_request(jsonl).unwrap().unwrap();

        assert_eq!(parsed.summary, "Need scope.");
        assert_eq!(parsed.questions[0].options, vec!["A", "B"]);
    }

    #[test]
    fn formats_comment_template() {
        let request = ClarificationRequest {
            request_type: "clarification_request".to_string(),
            blocking: true,
            summary: "Need scope.".to_string(),
            questions: vec![ClarificationQuestion {
                id: "scope".to_string(),
                question: "What scope?".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
            }],
        };

        let comment = format_clarification_comment("run-1", "clarification-1", &request);

        assert!(comment.contains("Sisyphus needs clarification"));
        assert!(comment.contains("1. What scope?"));
        assert!(comment.contains("   - A"));
        assert!(comment.contains("<!-- sisyphus:run=run-1; clarification=clarification-1 -->"));
    }
}
