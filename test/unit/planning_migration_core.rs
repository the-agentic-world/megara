use sha2::{Digest, Sha256};

use crate::planning::{
    domain::{LegacyContextBundle, LegacyContextEncoding, ModelActionKind, OpaqueLegacyFile},
    engine::{InMemoryPlanningCore, LegacyImportCommand},
};

fn command() -> LegacyImportCommand {
    let mut command = LegacyImportCommand {
        session_id: "pln_migration-session".to_string(),
        project_id: "prj_test".to_string(),
        initial_request: "opaque legacy request".to_string(),
        legacy_bundle: LegacyContextBundle {
            migration_id: "mig_test".to_string(),
            source_backup_id: "mig_test".to_string(),
            source_bundle_hash: "sha256:bundle".to_string(),
            source_path: ".agents/state/raw.json".to_string(),
            files: vec![OpaqueLegacyFile {
                relative_path: ".agents/state/raw.json".to_string(),
                byte_sha256:
                    "sha256:daf505a07aad0a2b8dae061126f3fe0bcc7f39668af36385e48da5a43067987f"
                        .to_string(),
                size: 4,
                encoding: LegacyContextEncoding::Hex,
                payload: "00ff0041".to_string(),
            }],
        },
    };
    command.legacy_bundle.source_bundle_hash = command.legacy_bundle.computed_source_bundle_hash();
    command
}

#[test]
fn legacy_import_is_one_interview_delta_event_with_opaque_context() {
    let mut core = InMemoryPlanningCore::default();
    let command = command();
    let expected_bundle = serde_json::to_value(&command.legacy_bundle).unwrap();
    let result = core.import_legacy(command.clone()).unwrap();
    let state = result.state;

    assert_eq!(core.events().len(), 1);
    assert_eq!(result.event.operation, "planning.migration.import");
    assert_eq!(result.event.seq, 1);
    assert_eq!(result.event.revision_after, 1);
    assert_eq!(result.event.domain_revision_after, 1);
    assert_eq!(
        state.phase,
        crate::planning::domain::LifecyclePhase::Interview
    );
    assert_eq!(state.revision, 1);
    assert_eq!(state.domain_revision, 1);
    assert_eq!(state.plan_revision, 0);
    assert!(state.imported_legacy_context);
    assert_eq!(
        state.legacy_import.as_ref().unwrap().migration_id,
        "mig_test"
    );
    assert_eq!(state.entities.revisions.len(), 0);
    assert!(state.spec.current_candidate.is_none());
    assert!(state.spec.approval.is_none());
    assert!(state.plan.current_candidate.is_none());
    assert!(state.plan.approval.is_none());
    let work_item = state.required_model_action.as_ref().unwrap();
    assert_eq!(work_item.kind, ModelActionKind::DeltaAudit);
    assert_eq!(work_item.context["legacy_context"], expected_bundle);
    assert_eq!(
        result.event.primary["command"]["legacy_bundle"],
        expected_bundle
    );
}

#[test]
fn legacy_import_is_deterministic_and_collision_is_zero_delta() {
    let command = command();
    let mut first = InMemoryPlanningCore::default();
    let first_result = first.import_legacy(command.clone()).unwrap();
    let first_events = first.events().to_vec();
    let first_state = first_result.state.clone();

    let mut equivalent = InMemoryPlanningCore::default();
    let equivalent_result = equivalent.import_legacy(command.clone()).unwrap();
    assert_eq!(equivalent_result.state, first_state);
    assert_eq!(equivalent.events(), first_events.as_slice());

    let error = first.import_legacy(command).unwrap_err();
    assert!(matches!(
        error,
        crate::planning::engine::CoreError::SessionExists(_)
    ));
    assert_eq!(first.events(), first_events.as_slice());
    assert_eq!(first.state("pln_migration-session"), Some(&first_state));
}

#[test]
fn legacy_import_rejects_oversized_opaque_bundle_without_event() {
    let mut core = InMemoryPlanningCore::default();
    let mut command = command();
    command.legacy_bundle.files[0].payload = "a".repeat(4 * 1024 * 1024);
    let error = core.import_legacy(command).unwrap_err();
    assert!(matches!(
        error,
        crate::planning::engine::CoreError::InvalidRequest(_)
    ));
    assert!(core.events().is_empty());
    assert!(core.state("pln_migration-session").is_none());
}

#[test]
fn legacy_bundle_validation_rejects_each_tamper_without_event() {
    let cases = [
        "hash",
        "size",
        "uppercase",
        "odd_hex",
        "non_hex",
        "path_parent",
        "path_absolute",
        "path_backslash",
        "path_nul",
        "duplicate",
        "unsorted",
    ];
    for case in cases {
        let mut command = command();
        match case {
            "hash" => {
                command.legacy_bundle.files[0].byte_sha256 =
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string()
            }
            "size" => command.legacy_bundle.files[0].size = 3,
            "uppercase" => command.legacy_bundle.files[0].payload = "00FF0041".to_string(),
            "odd_hex" => command.legacy_bundle.files[0].payload = "000".to_string(),
            "non_hex" => command.legacy_bundle.files[0].payload = "00gg".to_string(),
            "path_parent" => {
                command.legacy_bundle.files[0].relative_path = ".agents/../escape".to_string()
            }
            "path_absolute" => {
                command.legacy_bundle.files[0].relative_path = "/tmp/escape".to_string()
            }
            "path_backslash" => {
                command.legacy_bundle.files[0].relative_path = ".agents\\escape".to_string()
            }
            "path_nul" => {
                command.legacy_bundle.files[0].relative_path = ".agents/state/\0escape".to_string()
            }
            "duplicate" => {
                let duplicate = command.legacy_bundle.files[0].clone();
                command.legacy_bundle.files.push(duplicate);
            }
            "unsorted" => {
                let mut second = command.legacy_bundle.files[0].clone();
                second.relative_path = ".agents/state/z.json".to_string();
                command.legacy_bundle.files.insert(0, second);
            }
            _ => unreachable!(),
        }
        let mut core = InMemoryPlanningCore::default();
        let error = core.import_legacy(command).unwrap_err();
        assert!(
            matches!(error, crate::planning::engine::CoreError::InvalidRequest(_)),
            "case={case}"
        );
        assert!(core.events().is_empty(), "case={case}");
    }
}

#[test]
fn legacy_bundle_exact_decoded_limit_is_allowed_and_plus_one_is_rejected() {
    let bytes = vec![0u8; 4 * 1024 * 1024];
    let mut command = command();
    command.legacy_bundle.files[0].size = bytes.len() as u64;
    command.legacy_bundle.files[0].payload = bytes.iter().map(|_| "00").collect();
    command.legacy_bundle.files[0].byte_sha256 = digest(&bytes);
    command.legacy_bundle.source_bundle_hash = command.legacy_bundle.computed_source_bundle_hash();
    let mut core = InMemoryPlanningCore::default();
    assert!(core.import_legacy(command.clone()).is_ok());

    let mut oversized = command;
    oversized.legacy_bundle.files[0].size += 1;
    oversized.legacy_bundle.files[0].payload.push_str("00");
    oversized.legacy_bundle.source_bundle_hash =
        oversized.legacy_bundle.computed_source_bundle_hash();
    let mut rejected = InMemoryPlanningCore::default();
    assert!(rejected.import_legacy(oversized).is_err());
    assert!(rejected.events().is_empty());
}

#[test]
fn legacy_bundle_worst_case_file_and_path_bounds_stay_inside_event_limit() {
    let bytes = vec![0u8; 4 * 1024];
    let byte_sha256 = digest(&bytes);
    let payload = bytes.iter().map(|_| "00").collect::<String>();
    let mut files = Vec::with_capacity(1_024);
    for index in 0..1_024 {
        let prefix = format!(".agents/state/{index:04}-");
        let path = format!("{}{}", prefix, "a".repeat(1_024 - prefix.len()));
        files.push(OpaqueLegacyFile {
            relative_path: path,
            byte_sha256: byte_sha256.clone(),
            size: bytes.len() as u64,
            encoding: LegacyContextEncoding::Hex,
            payload: payload.clone(),
        });
    }
    let mut command = command();
    command.legacy_bundle.files = files;
    command.legacy_bundle.source_path = command.legacy_bundle.files[0].relative_path.clone();
    command.legacy_bundle.source_bundle_hash = command.legacy_bundle.computed_source_bundle_hash();
    let mut core = InMemoryPlanningCore::default();
    let result = core.import_legacy(command).unwrap();
    let event_size = serde_json::to_vec(&result.event).unwrap().len();
    assert!(event_size <= crate::planning::engine::LEGACY_EVENT_MAX_BYTES);
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[test]
fn legacy_import_reference_has_backward_cache_default_and_invariant_guard() {
    let state = crate::planning::domain::PlanningState::new(
        "pln_normal".to_string(),
        "prj_test".to_string(),
        "request".to_string(),
    );
    let mut value = serde_json::to_value(&state).unwrap();
    value.as_object_mut().unwrap().remove("legacy_import");
    let restored: crate::planning::domain::PlanningState = serde_json::from_value(value).unwrap();
    assert!(restored.legacy_import.is_none());
    assert!(restored.assert_invariants().is_ok());

    let mut invalid = state;
    invalid.imported_legacy_context = true;
    assert!(invalid.assert_invariants().is_err());
}
