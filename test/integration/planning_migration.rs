use std::{fs, path::Path};

use tempfile::tempdir;

use super::{
    planning::{
        self,
        domain::{EvidenceRecord, RepoEvidenceSnapshot},
        engine::EvidenceRefreshCommand,
        store::PlanningStore,
    },
    planning_migration_support as support,
};

#[test]
fn migration_dry_run_reports_relative_backup_remove_and_preserve_entries() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::install(project.path(), codex_home.path());
    support::seed_codex_legacy_projections(project.path());

    let first = support::run(project.path(), codex_home.path(), &["--dry-run", "--json"]);
    let report = support::report(&first);
    let encoded = String::from_utf8(first.stdout).unwrap();
    assert!(!encoded.contains(project.path().to_str().unwrap()));
    assert_eq!(
        support::entry_action(&report, ".codex/hooks.json", "remove")["action"],
        "remove"
    );
    assert_eq!(
        support::entry_action(&report, ".agents/skills/deep-interview/SKILL.md", "remove",)
            ["action"],
        "remove"
    );
    for path in [
        ".agents/skills/deep-interview/SKILL.md",
        ".agents/skills/ralplan/SKILL.md",
        ".agents/skills/team/SKILL.md",
        ".agents/skills/ultragoal/SKILL.md",
        ".codex/skills/deep-interview/SKILL.md",
        ".codex/skills/ralplan/SKILL.md",
        ".codex/skills/team/SKILL.md",
        ".codex/skills/ultragoal/SKILL.md",
        ".agents/skill-fragments/deep-interview/auto-answer-uncertain.md",
        ".agents/skill-fragments/deep-interview/auto-research-greenfield.md",
        ".agents/skill-fragments/deep-interview/lateral-review-panel.md",
        ".agents/skill-fragments/ultragoal/ai-slop-cleaner.md",
        ".codex/skill-fragments/deep-interview/auto-answer-uncertain.md",
        ".codex/skill-fragments/deep-interview/auto-research-greenfield.md",
        ".codex/skill-fragments/deep-interview/lateral-review-panel.md",
        ".codex/skill-fragments/ultragoal/ai-slop-cleaner.md",
    ] {
        assert_eq!(
            support::entry_action(&report, path, "remove")["action"],
            "remove",
            "{path}"
        );
    }
    assert!(report["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| { Path::new(entry["relative_path"].as_str().unwrap()).is_relative() }));

    let skill = project.path().join(".codex/skills/deep-interview/SKILL.md");
    let mut changed = fs::read(&skill).unwrap();
    changed.push(b'\n');
    fs::write(&skill, changed).unwrap();
    let changed_report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--dry-run", "--json"],
    ));
    let changed_entry = support::entry_action(
        &changed_report,
        ".codex/skills/deep-interview/SKILL.md",
        "preserve",
    );
    assert_eq!(changed_entry["action"], "preserve");
    assert_eq!(changed_entry["reason"], "not_managed");
}

#[test]
fn migration_apply_imports_opaque_context_with_system_core_event() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::install(project.path(), codex_home.path());
    let legacy_path = support::write_legacy_file(project.path(), b"\0legacy\n");

    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let session_id = report["session_id"].as_str().unwrap();
    let migration_id = report["migration_id"].as_str().unwrap();
    let store = PlanningStore::open_project(project.path()).unwrap();
    let state = store.current(session_id).unwrap();
    assert_eq!(state.phase, planning::domain::LifecyclePhase::Interview);
    assert!(state.imported_legacy_context);
    assert_eq!(state.revision, 1);
    assert_eq!(store.event_count(session_id).unwrap(), 1);
    let envelopes = store.event_envelopes(session_id).unwrap();
    assert_eq!(envelopes.len(), 1);
    let envelope = &envelopes[0];
    assert_eq!(
        envelope.event_type,
        planning::store::EventType::MigrationImport
    );
    assert_eq!(envelope.metadata.actor, planning::store::EventActor::System);
    assert_eq!(
        envelope.metadata.adapter,
        planning::store::EventAdapter::Core
    );
    assert_eq!(envelope.metadata.request_id, None);
    assert_eq!(
        envelope.semantic_payload.primary["command"]["legacy_bundle"]["files"][0]["payload"],
        "006c65676163790a"
    );
    assert_eq!(
        state.required_model_action.as_ref().unwrap().context["legacy_context"]["files"][0]
            ["payload"],
        "006c65676163790a"
    );
    assert!(!legacy_path.exists());
    assert!(project
        .path()
        .join(format!(
            ".megara/migration-backups/{migration_id}/manifest.json"
        ))
        .exists());
}

#[test]
fn managed_projection_only_migration_has_no_session_or_database() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::install(project.path(), codex_home.path());
    let db = project.path().join(".megara/planning/planning.db");

    let report = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    assert_eq!(report["session_id"], serde_json::Value::Null);
    assert!(!db.exists());
    assert!(!project.path().join(".codex/hooks.json").exists());
    assert_eq!(
        support::entry_action(&report, ".codex/hooks.json", "remove")["action"],
        "remove"
    );
}

#[test]
fn migration_rollback_restores_bytes_and_keeps_terminal_journal_readable() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy_path = support::write_legacy_file(project.path(), b"rollback\0bytes");
    let original = fs::read(&legacy_path).unwrap();

    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let rolled_back = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(rolled_back["phase"], "rolled_back");
    assert_eq!(fs::read(&legacy_path).unwrap(), original);
    let backup_root = project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}"));
    assert!(backup_root.join("manifest.json").exists());
    assert!(!backup_root.join("files").exists());

    let retry = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(retry["phase"], "rolled_back");
}

#[cfg(unix)]
#[test]
fn migration_rejects_symlinked_legacy_parents_without_touching_outside() {
    use std::os::unix::fs::symlink;

    for parent in [".agents", ".megara"] {
        let project = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("state/workflows/outside.json");
        fs::create_dir_all(outside_file.parent().unwrap()).unwrap();
        fs::write(&outside_file, b"outside-secret").unwrap();
        symlink(outside.path(), project.path().join(parent)).unwrap();

        let output = support::run(project.path(), outside.path(), &["--apply", "--json"]);
        assert!(!output.status.success(), "parent={parent}");
        assert_eq!(fs::read(&outside_file).unwrap(), b"outside-secret");
        assert!(!project.path().join(".megara/migration-backups").exists());
    }
}

#[test]
fn forced_rollback_exports_changed_session_and_still_protects_destination() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy_path = support::write_legacy_file(project.path(), b"rollback-force");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let mut store = PlanningStore::open_project(project.path()).unwrap();
    store
        .refresh_evidence(
            "cmd-migration-force-evidence",
            "sha256:migration-force-evidence",
            EvidenceRefreshCommand {
                session_id: session_id.clone(),
                expected_revision: 1,
                snapshot: RepoEvidenceSnapshot {
                    evidence_hash: "sha256:migration-force-snapshot".to_string(),
                    head_oid: None,
                    head_ref: None,
                    dirty: false,
                    status_hash: "sha256:migration-force-status".to_string(),
                    cited_files_hash: "sha256:migration-force-files".to_string(),
                    evidence: vec![EvidenceRecord {
                        evidence_id: "EVID-001".to_string(),
                        path: "src/main.rs".to_string(),
                        ranges: Vec::new(),
                        size: 1,
                        sha256: "sha256:evidence-file".to_string(),
                        tracked: true,
                        captured_at: "unix-nanos:1".to_string(),
                    }],
                },
            },
        )
        .unwrap();
    drop(store);

    fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    fs::write(&legacy_path, b"user-created-destination").unwrap();
    let protected = support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--force", "--json"],
    );
    assert!(!protected.status.success());
    assert_eq!(fs::read(&legacy_path).unwrap(), b"user-created-destination");
    assert!(!project
        .path()
        .join(format!(
            ".megara/migration-backups/{migration_id}/rollback-export.json"
        ))
        .exists());

    fs::remove_file(&legacy_path).unwrap();
    let forced = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--force", "--json"],
    ));
    assert_eq!(forced["phase"], "rolled_back");
    assert_eq!(fs::read(&legacy_path).unwrap(), b"rollback-force");
    assert!(project
        .path()
        .join(format!(
            ".megara/migration-backups/{migration_id}/rollback-export.json"
        ))
        .exists());
    let store = PlanningStore::open_project(project.path()).unwrap();
    let (cleanup_state, pending_backup_id): (String, Option<String>) = store
        .conn
        .query_row(
            "SELECT cleanup_state, pending_backup_id FROM purged_sessions WHERE session_id=?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(cleanup_state, "clean");
    assert_eq!(pending_backup_id, None);
}

#[test]
fn public_purge_removes_the_linked_migration_backup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"privacy");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let mut store = PlanningStore::open_project(project.path()).unwrap();
    let receipt = store
        .purge(
            &session_id,
            "cmd-public-migration-purge",
            "sha256:public-migration-purge",
            1,
            &session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "clean");
    assert!(!project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}"))
        .exists());
}

#[test]
fn rollback_retry_recovers_after_purge_and_after_terminal_journal_windows() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let legacy_path = support::write_legacy_file(project.path(), b"crash-window");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let mut store = PlanningStore::open_project(project.path()).unwrap();
    let project_id = store.project_id().to_string();
    let rollback_basis =
        serde_json::json!([project_id.clone(), migration_id.clone(), "rollback-purge"]);
    let rollback_command = format!(
        "cmd_mig_rollback_{}",
        planning::canonical::canonical_hash(&rollback_basis).trim_start_matches("sha256:")
    );
    let request_hash = planning::canonical::canonical_hash(&rollback_basis);
    store
        .purge_for_rollback(
            &session_id,
            &rollback_command,
            &request_hash,
            1,
            &session_id,
        )
        .unwrap();
    drop(store);
    assert!(!legacy_path.exists());
    assert!(project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}/files"))
        .exists());

    let recovered = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(recovered["phase"], "rolled_back");
    assert_eq!(fs::read(&legacy_path).unwrap(), b"crash-window");

    let backup_root = project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}"));
    fs::create_dir_all(backup_root.join("files")).unwrap();
    fs::write(backup_root.join("files/residue"), b"residue").unwrap();
    let terminal_retry = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--rollback", &migration_id, "--json"],
    ));
    assert_eq!(terminal_retry["phase"], "rolled_back");
    assert!(!backup_root.join("files").exists());
}

#[cfg(unix)]
#[test]
fn pending_linked_backup_cleanup_is_repaired_after_path_fault() {
    use std::os::unix::fs::symlink;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    support::write_legacy_file(project.path(), b"pending-cleanup");
    let applied = support::report(&support::run(
        project.path(),
        codex_home.path(),
        &["--apply", "--json"],
    ));
    let migration_id = applied["migration_id"].as_str().unwrap().to_string();
    let session_id = applied["session_id"].as_str().unwrap().to_string();
    let backup_root = project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}"));
    let held_root = project
        .path()
        .join(format!(".megara/migration-backups/{migration_id}-held"));
    fs::rename(&backup_root, &held_root).unwrap();
    symlink(&held_root, &backup_root).unwrap();

    let mut store = PlanningStore::open_project(project.path()).unwrap();
    let receipt = store
        .purge(
            &session_id,
            "cmd-pending-linked-cleanup",
            "sha256:pending-linked-cleanup",
            1,
            &session_id,
        )
        .unwrap();
    assert_eq!(receipt.cleanup_state, "pending");
    fs::remove_file(&backup_root).unwrap();
    fs::rename(&held_root, &backup_root).unwrap();
    assert_eq!(store.repair_pending_cleanup().unwrap(), 1);
    assert!(!backup_root.exists());
}
