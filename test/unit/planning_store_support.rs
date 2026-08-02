use crate::planning::{
    domain::RepoEvidenceSnapshot,
    engine::StartCommand,
    store::{PlanningStore, ProjectIdentity, StoredOutcome},
};
use tempfile::TempDir;

pub(crate) fn open_store() -> (TempDir, ProjectIdentity, PlanningStore) {
    let directory = tempfile::tempdir().unwrap();
    let identity = ProjectIdentity {
        canonical_root: directory.path().to_string_lossy().into_owned(),
        project_id: "prj_store_test".to_string(),
    };
    let path = directory.path().join("planning.db");
    let store = PlanningStore::open(&path, identity.clone()).unwrap();
    (directory, identity, store)
}

pub(crate) fn start(store: &mut PlanningStore, command_id: &str) -> StoredOutcome {
    let project_id = store.project_id().to_string();
    store
        .start(
            command_id,
            "sha256:request",
            StartCommand {
                session_id: Some("pln_store".to_string()),
                project_id,
                request: "저장과 재생을 검증한다.".to_string(),
                title: None,
            },
        )
        .unwrap()
}

pub(crate) fn snapshot(evidence_hash: &str) -> RepoEvidenceSnapshot {
    RepoEvidenceSnapshot {
        evidence_hash: evidence_hash.to_string(),
        head_oid: None,
        status_hash: format!("{evidence_hash}-status"),
        cited_files_hash: format!("{evidence_hash}-files"),
    }
}
