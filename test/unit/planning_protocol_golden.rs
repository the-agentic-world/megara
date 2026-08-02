use crate::planning::protocol::{LogicalRequest, PROTOCOL_VERSION};
use crate::planning::service::PlanningService;
use crate::planning::{
    domain::{AnswerMode, QuestionProposal, SourceRef},
    store::PlanningStore,
};
use serde_json::{json, Value};
use tempfile::tempdir;

#[test]
fn protocol_fixture_is_complete_and_slice_two_responses_conform() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string("test/fixtures/planning/protocol/operations-v1.json").unwrap(),
    )
    .unwrap();
    let vocabulary = [
        "array",
        "boolean",
        "boolean|null",
        "enum(complete|interview|planning|specification)",
        "enum(written|unchanged|conflict|io_error)",
        "integer",
        "object",
        "object|null",
        "string",
        "string|null",
    ];
    for envelope in ["request_envelope", "success_envelope", "observed"] {
        assert_descriptor(&fixture[envelope], &vocabulary);
    }
    let entries = fixture["operations"].as_array().unwrap();
    let mut names = std::collections::BTreeSet::new();
    for entry in entries {
        let name = entry["operation"].as_str().unwrap();
        assert!(names.insert(name), "duplicate operation descriptor: {name}");
        assert!(matches!(entry["kind"].as_str(), Some("mutation" | "query")));
        assert_descriptor(&entry["request"], &vocabulary);
        assert_descriptor(&entry["result"], &vocabulary);
        let request_fields = entry["request"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .chain(entry["request"]["optional"].as_array().unwrap());
        for field in request_fields {
            let field = field.as_str().unwrap();
            if field.starts_with("params.") {
                assert!(entry["request"]["param_types"].get(field).is_some());
            }
        }
    }
    assert_eq!(
        names.len(),
        crate::planning::protocol::LOGICAL_OPERATIONS.len()
    );

    let directory = tempdir().unwrap();
    let mut service = PlanningService::open_project(directory.path()).unwrap();
    let started = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-start".to_string(),
        operation: "planning.start".to_string(),
        command_id: Some("cmd-golden-start".to_string()),
        session_id: None,
        expected_revision: None,
        params: Some(json!({"request":"golden request"})),
    });
    assert_success_result(&fixture, &started);
    let session_id = started["session_id"].as_str().unwrap().to_string();
    for operation in ["planning.status", "planning.current"] {
        let response = service.handle_user_request(LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: format!("req-{operation}"),
            operation: operation.to_string(),
            command_id: None,
            session_id: Some(session_id.clone()),
            expected_revision: None,
            params: None,
        });
        assert_success_result(&fixture, &response);
    }
    let list = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-list".to_string(),
        operation: "planning.list".to_string(),
        command_id: None,
        session_id: None,
        expected_revision: None,
        params: None,
    });
    assert_success_result(&fixture, &list);

    let evidence = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-evidence".to_string(),
        operation: "planning.evidence.refresh".to_string(),
        command_id: Some("cmd-golden-evidence".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(1),
        params: Some(json!({"citations":[]})),
    });
    assert_success_result(&fixture, &evidence);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    let state = store.current(&session_id).unwrap();
    let work_item = state.required_model_action.clone().unwrap();
    drop(store);
    let proposal = QuestionProposal {
        context: "배경".to_string(),
        question: "무엇을 원하시나요?".to_string(),
        why_it_matters: "답에 따라 계획이 달라집니다.".to_string(),
        technical_terms: Vec::new(),
        source_refs: vec![SourceRef::InitialRequest {
            id: "request".to_string(),
        }],
        answer: AnswerMode::Freeform {
            freeform_hint: "답을 적어 주세요.".to_string(),
        },
    };
    let audit = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-audit".to_string(),
        operation: "planning.audit.apply".to_string(),
        command_id: Some("cmd-golden-audit".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(2),
        params: Some(json!({"mode":"delta","proposal":{
            "schema":"megara.audit-proposal/v1","mode":"delta",
            "work_item_id":work_item.work_item_id,
            "base_revision":work_item.base_revision,
            "base_domain_revision":work_item.base_domain_revision,
            "input_hash":work_item.input_hash,
            "readiness":"continue","next_question":proposal,
            "entity_ops":[],"edge_ops":[],"blocker_ops":[],"counterexample_review":null
        }})),
    });
    assert_success_result(&fixture, &audit);
    let store = PlanningStore::open_project(directory.path()).unwrap();
    let pending = store
        .current(&session_id)
        .unwrap()
        .pending_question
        .unwrap();
    let answer = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-answer".to_string(),
        operation: "planning.answer".to_string(),
        command_id: Some("cmd-golden-answer".to_string()),
        session_id: Some(session_id.clone()),
        expected_revision: Some(pending.based_on_revision),
        params: Some(json!({"question_id":pending.question_id, "text":"답"})),
    });
    assert_success_result(&fixture, &answer);

    let purged = service.handle_user_request(LogicalRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: "req-golden-purge".to_string(),
        operation: "planning.purge".to_string(),
        command_id: Some("cmd-golden-purge".to_string()),
        session_id: Some(session_id),
        expected_revision: Some(4),
        params: Some(json!({"confirm":started["session_id"]})),
    });
    assert_success_result(&fixture, &purged);
}

#[test]
fn implemented_request_descriptors_drive_required_forbidden_and_type_negative_matrix() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string("test/fixtures/planning/protocol/operations-v1.json").unwrap(),
    )
    .unwrap();
    let requests = vec![
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-start".to_string(),
            operation: "planning.start".to_string(),
            command_id: Some("cmd-start".to_string()),
            session_id: None,
            expected_revision: None,
            params: Some(json!({"request":"x", "title":"t"})),
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-answer".to_string(),
            operation: "planning.answer".to_string(),
            command_id: Some("cmd-answer".to_string()),
            session_id: Some("pln-1".to_string()),
            expected_revision: Some(1),
            params: Some(json!({"question_id":"qst-1", "text":"a", "selected_choice_ids":[]})),
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-status".to_string(),
            operation: "planning.status".to_string(),
            command_id: None,
            session_id: Some("pln-1".to_string()),
            expected_revision: None,
            params: None,
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-current".to_string(),
            operation: "planning.current".to_string(),
            command_id: None,
            session_id: Some("pln-1".to_string()),
            expected_revision: None,
            params: None,
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-list".to_string(),
            operation: "planning.list".to_string(),
            command_id: None,
            session_id: None,
            expected_revision: None,
            params: Some(json!({"phase":"interview"})),
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-evidence".to_string(),
            operation: "planning.evidence.refresh".to_string(),
            command_id: Some("cmd-evidence".to_string()),
            session_id: Some("pln-1".to_string()),
            expected_revision: Some(1),
            params: Some(json!({"citations":[]})),
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-audit".to_string(),
            operation: "planning.audit.apply".to_string(),
            command_id: Some("cmd-audit".to_string()),
            session_id: Some("pln-1".to_string()),
            expected_revision: Some(1),
            params: Some(json!({"mode":"delta", "proposal":{}})),
        },
        LogicalRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-purge".to_string(),
            operation: "planning.purge".to_string(),
            command_id: Some("cmd-purge".to_string()),
            session_id: Some("pln-1".to_string()),
            expected_revision: Some(1),
            params: Some(json!({"confirm":"pln-1"})),
        },
    ];
    for request in requests {
        let entry = fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["operation"] == request.operation)
            .unwrap();
        assert!(request.validate().is_ok(), "valid request rejected");
        let raw = serde_json::to_value(&request).unwrap();
        for required in entry["request"]["required"].as_array().unwrap() {
            let mut missing = raw.clone();
            remove_path(&mut missing, required.as_str().unwrap());
            assert!(decode_and_validate(missing).is_err(), "missing {required}");
        }
        for forbidden in entry["request"]["forbidden"].as_array().unwrap() {
            let mut extra = raw.clone();
            insert_path(&mut extra, forbidden.as_str().unwrap(), json!("forbidden"));
            assert!(decode_and_validate(extra).is_err(), "forbidden {forbidden}");
        }
        for (field, expected) in entry["request"]["param_types"].as_object().unwrap() {
            let mut wrong = raw.clone();
            let value = if expected == "boolean" {
                json!("wrong")
            } else {
                json!(false)
            };
            insert_path(&mut wrong, field, value);
            assert!(decode_and_validate(wrong).is_err(), "wrong type {field}");
        }
    }
}

fn decode_and_validate(raw: Value) -> Result<(), ()> {
    let mut raw = raw;
    if let Some(object) = raw.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    crate::planning::protocol::decode_jsonl_frame(&serde_json::to_vec(&raw).map_err(|_| ())?)
        .map(|_| ())
        .map_err(|_| ())
}

fn remove_path(value: &mut Value, path: &str) {
    let mut parts = path.split('.');
    let Some(first) = parts.next() else { return };
    if let Some(second) = parts.next() {
        if let Some(object) = value.get_mut(first).and_then(Value::as_object_mut) {
            object.remove(second);
        }
    } else if let Some(object) = value.as_object_mut() {
        object.remove(first);
    }
}

fn insert_path(value: &mut Value, path: &str, inserted: Value) {
    let mut parts = path.split('.');
    let Some(first) = parts.next() else { return };
    if let Some(second) = parts.next() {
        value
            .as_object_mut()
            .unwrap()
            .entry(first)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap()
            .insert(second.to_string(), inserted);
    } else {
        value
            .as_object_mut()
            .unwrap()
            .insert(first.to_string(), inserted);
    }
}

fn assert_descriptor(descriptor: &Value, vocabulary: &[&str]) {
    for group in ["required", "optional", "forbidden"] {
        assert!(descriptor[group].is_array(), "missing {group} descriptor");
    }
    let mut fields = std::collections::BTreeSet::new();
    for group in ["required", "optional", "forbidden"] {
        for field in descriptor[group].as_array().unwrap() {
            assert!(
                fields.insert(field.as_str().unwrap()),
                "duplicate or overlapping field: {field}"
            );
        }
    }
    let typed_fields = descriptor
        .get("types")
        .or_else(|| descriptor.get("param_types"))
        .unwrap()
        .as_object()
        .unwrap();
    let described_fields = descriptor["required"]
        .as_array()
        .unwrap()
        .iter()
        .chain(descriptor["optional"].as_array().unwrap())
        .map(|field| field.as_str().unwrap())
        .filter(|field| {
            descriptor
                .get("param_types")
                .is_none_or(|_| field.starts_with("params."))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let typed_names = typed_fields
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(described_fields, typed_names, "descriptor types drift");
    assert!(descriptor["forbidden"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| !typed_names.contains(field.as_str().unwrap())));
    if descriptor.get("param_types").is_some() {
        assert!(typed_fields
            .keys()
            .all(|field| field.starts_with("params.")));
    }
    for field in typed_fields.keys() {
        assert!(
            vocabulary.contains(&typed_fields[field].as_str().unwrap()),
            "unknown type {} for {}",
            typed_fields[field],
            field
        );
    }
}

fn assert_success_result(fixture: &Value, response: &Value) {
    assert_eq!(response["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(response["ok"], true);
    assert!(response["request_id"].is_string());
    assert!(response["replayed"].is_boolean());
    assert!(response["result"].is_object());
    assert_success_envelope(&fixture["success_envelope"], response);
    assert_observed(&fixture["observed"], &response["observed"]);
    let operation = response["result"]["operation"].as_str().unwrap();
    let descriptor = fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["operation"] == operation)
        .unwrap();
    let result = &response["result"];
    assert_eq!(result["schema"], fixture["result_schema"]);
    let required = descriptor["result"]["required"].as_array().unwrap();
    let optional = descriptor["result"]["optional"].as_array().unwrap();
    for field in required {
        let name = field.as_str().unwrap();
        assert!(result.get(name).is_some(), "missing result.{name}");
        assert_json_type(
            result.get(name).unwrap(),
            descriptor["result"]["types"][name].as_str().unwrap(),
        );
    }
    for field in optional {
        let name = field.as_str().unwrap();
        if let Some(value) = result.get(name) {
            assert_json_type(value, descriptor["result"]["types"][name].as_str().unwrap());
        }
    }
    let allowed = required
        .iter()
        .chain(optional)
        .map(|field| field.as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(result
        .as_object()
        .unwrap()
        .keys()
        .all(|key| allowed.contains(key.as_str())));
}

fn assert_success_envelope(descriptor: &Value, response: &Value) {
    let required = descriptor["required"].as_array().unwrap();
    let optional = descriptor["optional"].as_array().unwrap();
    for field in required {
        let name = field.as_str().unwrap();
        assert!(response.get(name).is_some(), "missing envelope.{name}");
        assert_json_type(
            response.get(name).unwrap(),
            descriptor["types"][name].as_str().unwrap(),
        );
    }
    for field in optional {
        let name = field.as_str().unwrap();
        if let Some(value) = response.get(name) {
            assert_json_type(value, descriptor["types"][name].as_str().unwrap());
        }
    }
    let allowed = required
        .iter()
        .chain(optional)
        .map(|field| field.as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(response
        .as_object()
        .unwrap()
        .keys()
        .all(|key| allowed.contains(key.as_str())));
}

fn assert_observed(descriptor: &Value, observed: &Value) {
    for field in descriptor["required"].as_array().unwrap() {
        assert!(observed.get(field.as_str().unwrap()).is_some());
    }
    assert_eq!(
        observed.as_object().unwrap().len(),
        descriptor["required"].as_array().unwrap().len()
    );
    for field in descriptor["required"].as_array().unwrap() {
        let name = field.as_str().unwrap();
        assert_json_type(
            observed.get(name).unwrap(),
            descriptor["types"][name].as_str().unwrap(),
        );
    }
}

fn assert_json_type(value: &Value, expected: &str) {
    let matches = match expected {
        "string" => value.is_string(),
        "string|null" => value.is_string() || value.is_null(),
        "integer" => value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "boolean|null" => value.is_boolean() || value.is_null(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "object|null" => value.is_object() || value.is_null(),
        expected if expected.starts_with("enum(") => {
            let choices = expected
                .strip_prefix("enum(")
                .and_then(|value| value.strip_suffix(')'))
                .unwrap_or_default();
            value
                .as_str()
                .is_some_and(|actual| choices.split('|').any(|choice| choice == actual))
        }
        _ => false,
    };
    assert!(matches, "expected {expected}, got {value}");
}
