use serde::Serialize;
use serde_json::Value;

pub(crate) fn project_entity_record(record: &mut Value) {
    let Some(body) = record.get_mut("body") else {
        return;
    };
    let Value::Object(body_object) = body else {
        return;
    };
    if body_object.len() != 1 {
        return;
    }
    let Some((_, plain_body)) = body_object.iter().next() else {
        return;
    };
    if !plain_body.is_object() {
        return;
    }
    *body = plain_body.clone();
}

pub(crate) fn entity_record_value(record: &impl Serialize) -> Value {
    let mut value = serde_json::to_value(record).expect("entity serialization is infallible");
    project_entity_record(&mut value);
    value
}
