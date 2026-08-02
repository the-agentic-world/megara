use serde_json::Value;

use super::super::domain::{project_entity_record, PlanningState};

pub(crate) fn project_state(state: &PlanningState) -> Value {
    let mut value =
        serde_json::to_value(state).expect("planning state serialization is infallible");
    if let Some(revisions) = value
        .get_mut("entities")
        .and_then(|entities| entities.get_mut("revisions"))
        .and_then(Value::as_object_mut)
    {
        for records in revisions.values_mut() {
            if let Some(records) = records.as_array_mut() {
                for record in records {
                    project_entity_record(record);
                }
            }
        }
    }
    value
}
