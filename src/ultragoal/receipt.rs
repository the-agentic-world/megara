use super::{model::*, store::sha256_hex, *};

pub(super) fn completion_receipt(
    plan: &UltragoalPlan,
    goal_id: &str,
    evidence: &str,
    quality_gate: &Value,
    timestamp: &str,
) -> Result<CompletionReceipt> {
    let quality_gate_raw = serde_json::to_string(quality_gate)?;
    let quality_gate_sha256 = sha256_hex(quality_gate_raw.as_bytes());
    let evidence_sha256 = sha256_hex(evidence.as_bytes());
    let receipt_seed = format!("{goal_id}\n{timestamp}\n{quality_gate_sha256}\n{evidence_sha256}");
    let receipt_hash = sha256_hex(receipt_seed.as_bytes());
    Ok(CompletionReceipt {
        schema_version: 1,
        receipt_id: format!("ug-{}", &receipt_hash[..16]),
        goal_id: goal_id.to_string(),
        verified_at: timestamp.to_string(),
        brief_sha256: Some(plan.brief_sha256.clone()),
        source_plan_sha256: plan
            .source
            .as_ref()
            .and_then(|source| source.ralplan_plan_sha256.clone()),
        quality_gate_sha256,
        evidence_sha256,
    })
}
