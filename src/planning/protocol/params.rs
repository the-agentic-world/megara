use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::super::domain::LifecyclePhase;
use super::super::engine::AuditMode;
use super::request::ProtocolError;

pub(crate) fn validate_typed_params(operation: &str, params: Value) -> Result<(), ProtocolError> {
    normalize_typed_params(operation, params).map(|_| ())
}

pub(crate) fn normalize_typed_params(
    operation: &str,
    params: Value,
) -> Result<Value, ProtocolError> {
    macro_rules! decode {
        ($type:ty) => {
            serde_json::from_value::<$type>(params.clone())
                .and_then(|parsed| serde_json::to_value(parsed))
                .map_err(|error| {
                    ProtocolError::InvalidRequest(format!("{operation} params: {error}"))
                })
        };
    }
    match operation {
        "planning.start" => decode!(StartParams),
        "planning.answer" => decode!(AnswerParams),
        "planning.status" | "planning.current" => decode!(EmptyParams),
        "planning.list" => decode!(ListParams),
        "planning.evidence.refresh" => decode!(EvidenceRefreshParams),
        "planning.audit.apply" => decode!(AuditApplyParams),
        "planning.spec.generate" | "planning.plan.generate" => {
            decode!(CandidateGenerateParams)
        }
        "planning.spec.show" | "planning.plan.show" => decode!(CandidateShowParams),
        "planning.spec.approve" => decode!(SpecApproveParams),
        "planning.spec.revise" | "planning.plan.revise" => decode!(CandidateReviseParams),
        "planning.plan.approve" => decode!(PlanApproveParams),
        "planning.export" => decode!(ExportParams),
        "planning.purge" => decode!(PurgeParams),
        _ => Err(ProtocolError::InvalidRequest(format!(
            "unknown operation: {operation}"
        ))),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StartParams {
    request: String,
    title: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AnswerParams {
    question_id: String,
    text: String,
    #[serde(default)]
    selected_choice_ids: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    phase: Option<LifecyclePhase>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CitationRange {
    start_line: u64,
    end_line: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Citation {
    temp_ref: String,
    path: String,
    ranges: Vec<CitationRange>,
    claim: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefreshParams {
    citations: Vec<Citation>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AuditApplyParams {
    mode: AuditMode,
    proposal: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateGenerateParams {
    proposal: Map<String, Value>,
    projection_policy: Option<ProjectionPolicy>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionPolicy {
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateShowParams {
    candidate_id: Option<String>,
    format: Option<ShowFormat>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ShowFormat {
    Markdown,
    Json,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpecApproveParams {
    candidate_id: String,
    semantic_hash: String,
    base_domain_revision: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanApproveParams {
    candidate_id: String,
    semantic_hash: String,
    base_plan_revision: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateReviseParams {
    candidate_id: String,
    text: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    out: String,
    format: ExportFormat,
    #[serde(default)]
    include_transcript: bool,
    #[serde(default)]
    force: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ExportFormat {
    Bundle,
    StateJson,
    EventsJsonl,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PurgeParams {
    confirm: String,
}
