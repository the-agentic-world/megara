use super::{flow, store::*, *};
use crate::cli::UltragoalCompleteGoalsArgs;

pub(super) fn run(paths: &UltragoalPaths, args: UltragoalCompleteGoalsArgs) -> Result<()> {
    let mut plan = read_plan_required(paths)?;
    let timestamp = timestamp();
    let Some(index) = flow::next_goal_index(&plan.goals, args.retry_failed) else {
        write_runtime_state(paths, &plan, "complete", false, &timestamp)?;
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "state": "complete",
                    "session_id": plan.session_id,
                    "next_goal": Value::Null,
                    "counts": flow::goal_counts(&plan.goals),
                }))?
            );
        } else {
            println!("ultragoal session {}: all goals complete", plan.session_id);
        }
        return Ok(());
    };

    let goal = &mut plan.goals[index];
    let was_active = goal.status == "active";
    if !was_active {
        goal.status = "active".to_string();
        goal.started_at.get_or_insert_with(|| timestamp.clone());
        goal.updated_at = timestamp.clone();
        plan.updated_at = timestamp.clone();
        append_ledger(
            paths,
            &json!({
                "timestamp": timestamp,
                "event": "goal_started",
                "session_id": plan.session_id,
                "goal_id": goal.id,
                "title": goal.title,
            }),
        )?;
        write_plan(paths, &plan)?;
    }
    write_runtime_state(paths, &plan, "active", true, &timestamp)?;
    let goal = &plan.goals[index];
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "state": if was_active { "resumed" } else { "started" },
                "session_id": plan.session_id,
                "next_goal": goal,
                "counts": flow::goal_counts(&plan.goals),
            }))?
        );
    } else {
        println!(
            "ultragoal next-action=execute-goal goal-id={}\ntitle={}\nobjective={}\ncheckpoint requires=architectReview:CLEAR+APPROVE,executorQa:passed,iteration:passed",
            goal.id, goal.title, goal.objective
        );
    }
    Ok(())
}
