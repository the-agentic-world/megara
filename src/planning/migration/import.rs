use std::str;

use anyhow::Result;
use serde_json::json;

use super::{backup, types::MigrationManifest};
use crate::planning::engine::LEGACY_EVENT_MAX_BYTES;
use crate::planning::{
    domain::{LegacyContextBundle, LegacyContextEncoding, OpaqueLegacyFile},
    engine::LegacyImportCommand,
    store::{EventContext, PlanningStore, StoredOutcome},
};

pub(crate) fn command(
    project: &std::path::Path,
    store: &PlanningStore,
    manifest: &MigrationManifest,
) -> Result<LegacyImportCommand> {
    let mut bundle = bundle_from_manifest(project, manifest)?;
    bundle.migration_id = manifest.migration_id.clone();
    bundle.source_backup_id = manifest.migration_id.clone();
    bundle.source_bundle_hash = manifest.source_bundle_hash.clone();
    let mut command = LegacyImportCommand {
        session_id: session_id(
            store.project_id(),
            &manifest.migration_id,
            &manifest.source_bundle_hash,
        ),
        project_id: store.project_id().to_string(),
        initial_request: format!(
            "Legacy planning context imported from {}",
            bundle.source_path
        ),
        legacy_bundle: bundle,
    };
    if serde_json::to_vec(&command)?.len() > LEGACY_EVENT_MAX_BYTES {
        for file in &mut command.legacy_bundle.files {
            if let LegacyContextEncoding::Utf8 = file.encoding {
                let bytes = file.payload.as_bytes();
                file.encoding = LegacyContextEncoding::Hex;
                file.payload = hex_encode(bytes);
            }
        }
    }
    Ok(command)
}

pub(crate) fn command_id(manifest: &MigrationManifest) -> String {
    format!(
        "cmd_mig_{}",
        digest(&json!([
            manifest.project_id,
            manifest.migration_id,
            "legacy-import",
            manifest.source_bundle_hash
        ]))
    )
}

pub(crate) fn request_hash(manifest: &MigrationManifest) -> String {
    manifest.source_bundle_hash.clone()
}

pub(crate) fn import(
    project: &std::path::Path,
    store: &mut PlanningStore,
    manifest: &MigrationManifest,
) -> Result<StoredOutcome> {
    let command = command(project, store, manifest)?;
    let outcome = store.import_legacy_with_context(
        &manifest.import_command_id,
        &request_hash(manifest),
        command,
        EventContext::default(),
    )?;
    Ok(outcome)
}

fn bundle_from_manifest(
    project: &std::path::Path,
    manifest: &MigrationManifest,
) -> Result<LegacyContextBundle> {
    let first_opaque = manifest
        .files
        .iter()
        .find(|file| file.kind == "opaque")
        .ok_or_else(|| anyhow::anyhow!("migration has no opaque legacy context"))?;
    let files = manifest
        .files
        .iter()
        .filter(|file| file.kind == "opaque")
        .map(|file| {
            let bytes = backup::read(project, &manifest.migration_id, file)?;
            let (encoding, payload) = match str::from_utf8(&bytes) {
                Ok(value) if bytes.iter().all(|byte| *byte >= 0x20) => {
                    (LegacyContextEncoding::Utf8, value.to_string())
                }
                Err(_) => (LegacyContextEncoding::Hex, hex_encode(&bytes)),
                _ => (LegacyContextEncoding::Hex, hex_encode(&bytes)),
            };
            Ok(OpaqueLegacyFile {
                relative_path: file.relative_path.clone(),
                byte_sha256: file.sha256.clone(),
                size: file.size,
                encoding,
                payload,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(LegacyContextBundle {
        migration_id: manifest.migration_id.clone(),
        source_backup_id: manifest.migration_id.clone(),
        source_bundle_hash: manifest.source_bundle_hash.clone(),
        source_path: first_opaque.relative_path.clone(),
        files,
    })
}

fn session_id(project_id: &str, migration_id: &str, source_hash: &str) -> String {
    format!(
        "pln_mig_{}",
        digest(&json!([
            project_id,
            migration_id,
            "legacy-session",
            source_hash
        ]))
    )
}

fn digest(value: &serde_json::Value) -> String {
    crate::planning::canonical::canonical_hash(value)
        .trim_start_matches("sha256:")
        .to_string()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
