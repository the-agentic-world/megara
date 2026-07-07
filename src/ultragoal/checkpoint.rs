use super::{checkpoint_ledger, flow, receipt, store::*, *};
use crate::cli::{UltragoalCheckpointArgs, UltragoalGoalStatusArg};

impl UltragoalGoalStatusArg {
    fn as_status(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::ReviewBlocked => "review_blocked",
            Self::Superseded => "superseded",
        }
    }
}

pub(super) fn run(paths: &UltragoalPaths, args: UltragoalCheckpointArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    let index = plan
        .goals
        .iter()
        .position(|goal| goal.id == args.goal_id)
        .with_context(|| format!("unknown ultragoal goal id: {}", args.goal_id))?;
    let status = args.status.as_status();
    if args.evidence.trim().is_empty() {
        bail!("checkpoint evidence is required");
    }
    if status == "complete" && plan.goals[index].status != "active" {
        bail!(
            "complete checkpoints require an active goal; run `MEGARA_BIN=\"${{MEGARA_BIN:-.agents/bin/megara}}\"; \"$MEGARA_BIN\" ultragoal start-goal` first"
        );
    }

    let completion_receipt = if status == "complete" {
        let raw = args
            .quality_gate_json
            .as_deref()
            .context("--quality-gate-json is required for complete checkpoints")?;
        let quality_gate = read_quality_gate(raw)?;
        let artifact_root = env::current_dir().context("failed to read current directory")?;
        validate_quality_gate(&quality_gate, &artifact_root)?;
        Some(receipt::completion_receipt(
            &plan,
            &plan.goals[index].id,
            &args.evidence,
            &quality_gate,
            &timestamp,
        )?)
    } else {
        None
    };

    let receipt_for_ledger = completion_receipt.clone();
    update_goal(
        &mut plan,
        index,
        status,
        &args.evidence,
        completion_receipt,
        &timestamp,
    );
    let next_started = (status == "complete")
        .then(|| flow::start_next_pending_goal(&mut plan, &timestamp))
        .flatten();
    plan.updated_at = timestamp.clone();
    write_plan(paths, &plan)?;
    let (runtime_phase, runtime_active) = flow::runtime_phase_for_plan(&plan);
    write_runtime_state(paths, &plan, runtime_phase, runtime_active, &timestamp)?;
    checkpoint_ledger::record(checkpoint_ledger::RecordInput {
        paths,
        plan: &plan,
        goal_id: &args.goal_id,
        status,
        evidence: &args.evidence,
        receipt: &receipt_for_ledger,
        next_started: &next_started,
        timestamp: &timestamp,
    })?;

    let goal = &plan.goals[index];
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "session_id": plan.session_id,
                "goal": goal,
                "next_goal_started": next_started,
                "counts": flow::goal_counts(&plan.goals),
            }))?
        );
    } else if let Some(next) = next_started {
        println!(
            "ultragoal checkpoint recorded for {} ({status}); next active goal: {} - {}",
            goal.id, next["id"], next["title"]
        );
    } else {
        println!("ultragoal checkpoint recorded for {} ({status})", goal.id);
    }
    Ok(())
}

fn update_goal(
    plan: &mut model::UltragoalPlan,
    index: usize,
    status: &str,
    evidence: &str,
    receipt: Option<model::CompletionReceipt>,
    timestamp: &str,
) {
    let goal = &mut plan.goals[index];
    goal.status = status.to_string();
    goal.evidence = Some(evidence.to_string());
    goal.updated_at = timestamp.to_string();
    if status == "active" {
        goal.started_at.get_or_insert_with(|| timestamp.to_string());
    }
    if status == "complete" {
        goal.completed_at = Some(timestamp.to_string());
        goal.completion_receipt = receipt;
    }
}
