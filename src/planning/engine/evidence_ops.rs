use std::collections::BTreeSet;

use super::super::domain::*;
use super::CoreError;

pub(crate) fn validate_snapshot(snapshot: &RepoEvidenceSnapshot) -> Result<(), CoreError> {
    if snapshot.evidence_hash.trim().is_empty()
        || snapshot.status_hash.trim().is_empty()
        || snapshot.cited_files_hash.trim().is_empty()
    {
        return Err(CoreError::InvalidRequest(
            "evidence snapshot hashes must not be blank".to_string(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in &snapshot.evidence {
        if !record.evidence_id.starts_with("EVID-")
            || !ids.insert(&record.evidence_id)
            || record.path.trim().is_empty()
            || !record.captured_at.starts_with("unix-nanos:")
            || !record.captured_at["unix-nanos:".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(CoreError::InvalidRequest(
                "evidence records must have canonical IDs and timestamps".to_string(),
            ));
        }
        let mut ranges = BTreeSet::new();
        for range in &record.ranges {
            if range.start_line == 0
                || range.end_line < range.start_line
                || !ranges.insert((range.start_line, range.end_line))
            {
                return Err(CoreError::InvalidRequest(
                    "evidence ranges must be unique 1-based inclusive ranges".to_string(),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn evidence_changes(
    previous: Option<&RepoEvidenceSnapshot>,
    current: &RepoEvidenceSnapshot,
) -> (BTreeSet<String>, bool) {
    let Some(previous) = previous else {
        return (
            current
                .evidence
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect(),
            false,
        );
    };
    let broad = previous.head_oid != current.head_oid
        || previous.head_ref != current.head_ref
        || previous.dirty != current.dirty
        || previous.status_hash != current.status_hash;
    let mut ids = BTreeSet::new();
    for old in &previous.evidence {
        let Some(new) = current
            .evidence
            .iter()
            .find(|record| record.evidence_id == old.evidence_id)
        else {
            ids.insert(old.evidence_id.clone());
            continue;
        };
        if old.path != new.path
            || old.ranges != new.ranges
            || old.size != new.size
            || old.sha256 != new.sha256
            || old.tracked != new.tracked
        {
            ids.insert(old.evidence_id.clone());
        }
    }
    ids.extend(
        current
            .evidence
            .iter()
            .filter(|record| {
                !previous
                    .evidence
                    .iter()
                    .any(|old| old.evidence_id == record.evidence_id)
            })
            .map(|record| record.evidence_id.clone()),
    );
    (ids, broad)
}
