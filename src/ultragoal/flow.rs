use super::{model::*, *};

pub(super) fn next_goal_index(goals: &[UltragoalGoal], retry_failed: bool) -> Option<usize> {
    goals
        .iter()
        .position(|goal| goal.status == "active")
        .or_else(|| goals.iter().position(|goal| goal.status == "pending"))
        .or_else(|| retry_failed.then(|| goals.iter().position(|goal| goal.status == "failed"))?)
}

pub(super) fn start_next_pending_goal(plan: &mut UltragoalPlan, timestamp: &str) -> Option<Value> {
    if plan.goals.iter().any(|goal| goal.status == "active") {
        return None;
    }
    let index = plan
        .goals
        .iter()
        .position(|goal| goal.status == "pending")?;
    let goal = &mut plan.goals[index];
    goal.status = "active".to_string();
    goal.started_at.get_or_insert_with(|| timestamp.to_string());
    goal.updated_at = timestamp.to_string();
    Some(json!({
        "id": goal.id,
        "title": goal.title,
        "objective": goal.objective,
    }))
}

pub(super) fn goal_counts(goals: &[UltragoalGoal]) -> GoalCounts {
    goals
        .iter()
        .fold(GoalCounts::default(), |mut counts, goal| {
            match goal.status.as_str() {
                "pending" => counts.pending += 1,
                "active" => counts.active += 1,
                "complete" => counts.complete += 1,
                "failed" => counts.failed += 1,
                "blocked" => counts.blocked += 1,
                "review_blocked" => counts.review_blocked += 1,
                "superseded" => counts.superseded += 1,
                _ => {}
            }
            counts
        })
}

pub(super) fn runtime_phase_for_plan(plan: &UltragoalPlan) -> (&'static str, bool) {
    let counts = goal_counts(&plan.goals);
    if counts.active > 0 {
        ("active", true)
    } else if counts.pending > 0 {
        ("goal_planning", true)
    } else if counts.blocked + counts.review_blocked > 0 {
        ("blocked", true)
    } else if counts.failed > 0 {
        ("failed", true)
    } else {
        ("complete", false)
    }
}
