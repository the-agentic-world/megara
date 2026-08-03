use std::fs;
use std::path::Path;
use std::process::Command;

use crate::planning::domain::{EvidenceRange, RepoEvidenceSnapshot};
use crate::planning::evidence::{
    capture_snapshot, capture_snapshot_with_previous, snapshot_is_current, EvidenceCitation,
    EvidenceError, EVIDENCE_CITATIONS_SCHEMA,
};
use tempfile::TempDir;

fn citation(path: &str, ranges: Vec<EvidenceRange>) -> EvidenceCitation {
    EvidenceCitation {
        temp_ref: path.replace('/', "_"),
        path: path.to_string(),
        ranges,
        claim: "구조를 확인한다.".to_string(),
    }
}

fn git_project() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    Command::new("git")
        .args(["init", "-q", "--initial-branch", "main"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.invalid"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Evidence Test"])
        .current_dir(root)
        .status()
        .unwrap();
    directory
}

fn commit(root: &Path) {
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-q", "-m", "fixture"])
        .current_dir(root)
        .status()
        .unwrap();
}

#[test]
fn non_git_snapshot_allows_empty_citations_and_records_identity() {
    let directory = tempfile::tempdir().unwrap();
    let snapshot = capture_snapshot(directory.path(), &[]).unwrap();
    assert!(snapshot.head_oid.is_none());
    assert!(snapshot.head_ref.is_none());
    assert!(!snapshot.dirty);
    assert!(snapshot.evidence.is_empty());
    assert!(snapshot.evidence_hash.starts_with("sha256:"));
}

#[test]
fn ranges_are_one_based_inclusive_and_empty_means_full_file() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("source.rs"), "a\nb").unwrap();
    let full = capture_snapshot(directory.path(), &[citation("source.rs", Vec::new())]).unwrap();
    assert!(full.evidence[0].ranges.is_empty());
    let ranged = capture_snapshot(
        directory.path(),
        &[citation(
            "source.rs",
            vec![
                EvidenceRange {
                    start_line: 1,
                    end_line: 1,
                },
                EvidenceRange {
                    start_line: 2,
                    end_line: 2,
                },
            ],
        )],
    )
    .unwrap();
    assert_eq!(ranged.evidence[0].ranges.len(), 2);
    for ranges in [
        vec![EvidenceRange {
            start_line: 0,
            end_line: 1,
        }],
        vec![EvidenceRange {
            start_line: 2,
            end_line: 3,
        }],
        vec![
            EvidenceRange {
                start_line: 1,
                end_line: 1,
            },
            EvidenceRange {
                start_line: 1,
                end_line: 1,
            },
        ],
    ] {
        assert!(matches!(
            capture_snapshot(directory.path(), &[citation("source.rs", ranges)]),
            Err(EvidenceError::InvalidRange(_)) | Err(EvidenceError::InvalidRequest(_))
        ));
    }
}

#[test]
fn stable_evidence_ids_survive_change_addition_and_reordering() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a.rs"), "a").unwrap();
    fs::write(directory.path().join("b.rs"), "b").unwrap();
    let first = capture_snapshot(directory.path(), &[citation("a.rs", Vec::new())]).unwrap();
    fs::write(directory.path().join("a.rs"), "changed").unwrap();
    let changed = capture_snapshot_with_previous(
        directory.path(),
        &[citation("a.rs", Vec::new()), citation("b.rs", Vec::new())],
        Some(&first),
    )
    .unwrap();
    assert_eq!(changed.evidence[0].evidence_id, "EVID-001");
    assert_eq!(changed.evidence[1].evidence_id, "EVID-002");
    let reordered = capture_snapshot_with_previous(
        directory.path(),
        &[citation("b.rs", Vec::new()), citation("a.rs", Vec::new())],
        Some(&changed),
    )
    .unwrap();
    assert!(reordered.semantic_eq(&changed));
    assert_eq!(reordered.evidence[0].evidence_id, "EVID-001");
    assert_eq!(reordered.evidence[1].evidence_id, "EVID-002");
}

#[test]
fn git_identity_clean_dirty_and_unborn_are_distinct_and_megara_is_excluded() {
    let directory = git_project();
    let root = directory.path();
    fs::write(root.join("source.rs"), "a").unwrap();
    let unborn = capture_snapshot(root, &[citation("source.rs", Vec::new())]).unwrap();
    assert!(unborn.head_oid.is_none());
    assert_eq!(unborn.head_ref.as_deref(), Some("main"));
    commit(root);
    let clean = capture_snapshot(root, &[citation("source.rs", Vec::new())]).unwrap();
    assert!(clean.head_oid.is_some());
    assert!(!clean.dirty);
    fs::write(root.join("source.rs"), "changed").unwrap();
    let dirty = capture_snapshot(root, &[citation("source.rs", Vec::new())]).unwrap();
    assert!(dirty.dirty);
    let status_before = dirty.status_hash.clone();
    fs::create_dir_all(root.join(".megara")).unwrap();
    fs::write(root.join(".megara/ignored-state"), "metadata").unwrap();
    let status_after = capture_snapshot(root, &[citation("source.rs", Vec::new())]).unwrap();
    assert_eq!(status_after.status_hash, status_before);
}

#[test]
fn missing_cited_file_is_a_stale_query_not_a_query_failure() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("source.rs"), "source").unwrap();
    let snapshot =
        capture_snapshot(directory.path(), &[citation("source.rs", Vec::new())]).unwrap();
    fs::remove_file(directory.path().join("source.rs")).unwrap();
    assert!(!snapshot_is_current(directory.path(), &snapshot).unwrap());
}

#[test]
fn citation_request_schema_is_exact() {
    let value = serde_json::json!({
        "schema": EVIDENCE_CITATIONS_SCHEMA,
        "base_revision": 1,
        "citations": []
    });
    let request: crate::planning::evidence::EvidenceCitationRequest =
        serde_json::from_value(value).unwrap();
    assert_eq!(request.schema, EVIDENCE_CITATIONS_SCHEMA);
}

#[test]
fn snapshot_equality_keeps_timestamp_but_semantic_equality_ignores_it() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("source.rs"), "source").unwrap();
    let first = capture_snapshot(directory.path(), &[citation("source.rs", Vec::new())]).unwrap();
    let mut second: RepoEvidenceSnapshot = first.clone();
    second.evidence[0].captured_at = "unix-nanos:999".to_string();
    assert_ne!(first, second);
    assert!(first.semantic_eq(&second));
}

#[test]
fn freshness_detects_head_status_and_cited_bytes_but_scopes_nested_status() {
    let directory = git_project();
    let root = directory.path();
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("nested/source.rs"), "source\n").unwrap();
    fs::write(root.join("outside.rs"), "outside\n").unwrap();
    commit(root);
    let clean =
        capture_snapshot(root.join("nested"), &[citation("source.rs", Vec::new())]).unwrap();
    assert!(snapshot_is_current(root.join("nested"), &clean).unwrap());

    fs::write(root.join("outside.rs"), "outside changed\n").unwrap();
    assert!(snapshot_is_current(root.join("nested"), &clean).unwrap());

    commit(root);
    assert!(!snapshot_is_current(root.join("nested"), &clean).unwrap());

    let after_head =
        capture_snapshot(root.join("nested"), &[citation("source.rs", Vec::new())]).unwrap();
    fs::write(root.join("outside.rs"), "outside dirty\n").unwrap();
    assert!(snapshot_is_current(root.join("nested"), &after_head).unwrap());

    fs::write(root.join("nested/source.rs"), "source changed\n").unwrap();
    assert!(!snapshot_is_current(root.join("nested"), &after_head).unwrap());

    Command::new("git")
        .args(["checkout", "-q", "--detach"])
        .current_dir(root)
        .status()
        .unwrap();
    let detached =
        capture_snapshot(root.join("nested"), &[citation("source.rs", Vec::new())]).unwrap();
    assert!(detached.head_oid.is_some());
    assert!(detached.head_ref.is_none());
}

#[test]
fn evidence_ids_preserve_gaps_and_allocate_new_ids_after_removal() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["a.rs", "b.rs", "c.rs"] {
        fs::write(directory.path().join(name), name).unwrap();
    }
    let first = capture_snapshot(
        directory.path(),
        &[
            citation("a.rs", Vec::new()),
            citation("b.rs", Vec::new()),
            citation("c.rs", Vec::new()),
        ],
    )
    .unwrap();
    let gapped = capture_snapshot_with_previous(
        directory.path(),
        &[citation("a.rs", Vec::new()), citation("c.rs", Vec::new())],
        Some(&first),
    )
    .unwrap();
    assert_eq!(
        gapped
            .evidence
            .iter()
            .map(|record| record.evidence_id.as_str())
            .collect::<Vec<_>>(),
        ["EVID-001", "EVID-003"]
    );
    assert!(snapshot_is_current(directory.path(), &gapped).unwrap());

    let readded = capture_snapshot_with_previous(
        directory.path(),
        &[
            citation("a.rs", Vec::new()),
            citation("b.rs", Vec::new()),
            citation("c.rs", Vec::new()),
        ],
        Some(&gapped),
    )
    .unwrap();
    assert_eq!(
        readded
            .evidence
            .iter()
            .map(|record| record.evidence_id.as_str())
            .collect::<Vec<_>>(),
        ["EVID-001", "EVID-003", "EVID-004"]
    );
}

#[test]
fn evidence_hash_includes_non_git_root_but_ignores_capture_timestamp() {
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    fs::write(left.path().join("source.rs"), "same\n").unwrap();
    fs::write(right.path().join("source.rs"), "same\n").unwrap();
    let left_snapshot =
        capture_snapshot(left.path(), &[citation("source.rs", Vec::new())]).unwrap();
    let right_snapshot =
        capture_snapshot(right.path(), &[citation("source.rs", Vec::new())]).unwrap();
    assert_ne!(left_snapshot.evidence_hash, right_snapshot.evidence_hash);
    assert!(left_snapshot.semantic_eq(&left_snapshot.clone()));
    let mut timestamp_changed = left_snapshot.clone();
    timestamp_changed.evidence[0].captured_at = "unix-nanos:999999".to_string();
    assert_ne!(left_snapshot, timestamp_changed);
    assert!(left_snapshot.semantic_eq(&timestamp_changed));
    assert_eq!(left_snapshot.evidence_hash, timestamp_changed.evidence_hash);
}

#[test]
fn line_count_boundaries_are_exact_at_eof() {
    let directory = tempfile::tempdir().unwrap();
    let cases = [
        ("empty", "", 0),
        ("one", "a", 1),
        ("newline", "a\n", 1),
        ("two", "a\nb", 2),
    ];
    for (name, contents, lines) in cases {
        fs::write(directory.path().join(name), contents).unwrap();
        let within_eof = citation(
            name,
            vec![EvidenceRange {
                start_line: lines,
                end_line: lines,
            }],
        );
        if lines == 0 {
            assert!(matches!(
                capture_snapshot(directory.path(), &[within_eof]),
                Err(EvidenceError::InvalidRange(_))
            ));
        } else {
            assert!(capture_snapshot(directory.path(), &[within_eof]).is_ok());
            let beyond = citation(
                name,
                vec![EvidenceRange {
                    start_line: lines + 1,
                    end_line: lines + 1,
                }],
            );
            assert!(matches!(
                capture_snapshot(directory.path(), &[beyond]),
                Err(EvidenceError::InvalidRange(_))
            ));
        }
    }
}
