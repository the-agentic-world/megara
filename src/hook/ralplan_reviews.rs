use super::*;

pub(super) fn ready(state: &Value) -> bool {
    let Some(reviews) = state.get("reviews").and_then(Value::as_array) else {
        return false;
    };
    let mut latest = BTreeMap::<&str, &str>::new();
    for review in reviews {
        let Some(role) = review.get("role").and_then(Value::as_str) else {
            continue;
        };
        let Some(verdict) = review.get("verdict").and_then(Value::as_str) else {
            continue;
        };
        latest.insert(role, verdict);
    }

    let planner_ready = latest
        .get("planner")
        .is_some_and(|verdict| matches!(*verdict, "CLEAR" | "WATCH" | "OKAY"));
    let architect_ready = latest
        .get("architect")
        .is_some_and(|verdict| matches!(*verdict, "CLEAR" | "WATCH" | "OKAY"));
    let critic_ready = latest
        .get("critic")
        .is_some_and(|verdict| matches!(*verdict, "OKAY"));

    planner_ready && architect_ready && critic_ready
}
