use super::{flow::goal_counts, model::*, store::*, *};
use crate::cli::UltragoalStatusArgs;

pub(super) fn run(
    paths: &UltragoalPaths,
    session_id: &str,
    args: UltragoalStatusArgs,
) -> Result<()> {
    let Some(plan) = read_plan(paths)? else {
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "session_id": session_id,
                    "path": paths.dir.display().to_string(),
                    "evidence_dir": paths.evidence_dir.display().to_string(),
                    "state": "missing",
                    "counts": GoalCounts::default(),
                    "active_goal": Value::Null,
                    "goals": [],
                }))?
            );
        } else {
            println!(
                "ultragoal session {session_id}: no goals created ({})",
                paths.dir.display()
            );
        }
        return Ok(());
    };

    let report = StatusReport {
        session_id: &plan.session_id,
        path: &paths.dir,
        evidence_dir: &paths.evidence_dir,
        state: "ready",
        counts: goal_counts(&plan.goals),
        active_goal: plan.goals.iter().find(|goal| goal.status == "active"),
        goals: &plan.goals,
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let active = report
            .active_goal
            .map(|goal| format!("{} - {}", goal.id, goal.title))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "ultragoal session {}: active={}, pending={}, complete={}, failed={}, blocked={}",
            report.session_id,
            active,
            report.counts.pending,
            report.counts.complete,
            report.counts.failed,
            report.counts.blocked + report.counts.review_blocked
        );
    }
    Ok(())
}
