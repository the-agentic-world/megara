use super::{flow, model::*, store::*, *};
use crate::cli::{UltragoalSteerArgs, UltragoalSteerKindArg};

pub(super) fn run(paths: &UltragoalPaths, args: UltragoalSteerArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    match args.kind {
        UltragoalSteerKindArg::AddSubgoal => add_subgoal(paths, &mut plan, args, &timestamp),
        UltragoalSteerKindArg::AnnotateLedger => annotate_ledger(paths, &plan, args, &timestamp),
    }
}

fn add_subgoal(
    paths: &UltragoalPaths,
    plan: &mut UltragoalPlan,
    args: UltragoalSteerArgs,
    timestamp: &str,
) -> Result<()> {
    let title = required_arg(
        args.title.as_deref(),
        "--title is required for --kind add-subgoal",
    )?;
    let objective = required_arg(
        args.objective.as_deref(),
        "--objective is required for --kind add-subgoal",
    )?;
    let goal = UltragoalGoal {
        id: format!("G{:03}", plan.goals.len() + 1),
        title: title.to_string(),
        objective: objective.to_string(),
        status: "pending".to_string(),
        created_at: timestamp.to_string(),
        updated_at: timestamp.to_string(),
        started_at: None,
        completed_at: None,
        evidence: args.evidence.clone(),
        completion_receipt: None,
    };
    plan.goals.push(goal.clone());
    plan.updated_at = timestamp.to_string();
    write_plan(paths, plan)?;
    let (runtime_phase, runtime_active) = flow::runtime_phase_for_plan(plan);
    write_runtime_state(paths, plan, runtime_phase, runtime_active, timestamp)?;
    append_ledger(
        paths,
        &json!({
            "timestamp": timestamp,
            "event": "goal_added",
            "session_id": plan.session_id,
            "goal": goal.clone(),
            "rationale": args.rationale,
            "evidence": args.evidence,
        }),
    )?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&goal)?);
    } else {
        println!("ultragoal added {} - {}", goal.id, goal.title);
    }
    Ok(())
}

fn annotate_ledger(
    paths: &UltragoalPaths,
    plan: &UltragoalPlan,
    args: UltragoalSteerArgs,
    timestamp: &str,
) -> Result<()> {
    let evidence = required_arg(
        args.evidence.as_deref(),
        "--evidence is required for --kind annotate-ledger",
    )?;
    append_ledger(
        paths,
        &json!({
            "timestamp": timestamp,
            "event": "ledger_annotation",
            "session_id": plan.session_id,
            "rationale": args.rationale,
            "evidence": evidence,
        }),
    )?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "event": "ledger_annotation",
                "session_id": plan.session_id,
                "evidence": evidence,
            }))?
        );
    } else {
        println!("ultragoal ledger annotation recorded");
    }
    Ok(())
}

fn required_arg<'a>(value: Option<&'a str>, message: &'static str) -> Result<&'a str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context(message)
}
