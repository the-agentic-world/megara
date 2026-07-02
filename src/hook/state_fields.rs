use serde_json::Value;

pub(crate) const PLAN_FIELDS: &[&str] = &[
    "plan_gate",
    "plan_path",
    "plan_sha256",
    "plan_persisted_at",
    "plan_payload",
    "input_spec_path",
    "input_spec_sha256",
    "input_spec_persisted_at",
];

pub(crate) fn remove_state_fields(state: &mut Value, fields: &[&str]) {
    let Some(object) = state.as_object_mut() else {
        return;
    };
    for field in fields {
        object.remove(*field);
    }
}
