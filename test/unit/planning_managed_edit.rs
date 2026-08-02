use super::*;
use tempfile::tempdir;

#[test]
fn managed_edit_race_rejects_without_backup_or_temp_residue() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let backup = directory.path().join("config.toml.megara.mcp.bak");
    let original = b"original = true\n";
    fs::write(&path, original).unwrap();
    let permissions = fs::metadata(&path).unwrap().permissions();
    let edit = installer::ManagedTomlEdit {
        path: path.clone(),
        created: false,
        changed: true,
        backup_path: Some(backup.clone()),
        desired: "original = false\n".to_string(),
        backup: Some(original.to_vec()),
        expected_source: Some(original.to_vec()),
        permissions: Some(permissions),
    };
    let prepared = edit.prepare().unwrap();
    fs::write(&path, b"changed by another writer\n").unwrap();

    let error = prepared.commit().unwrap_err().to_string();
    assert!(error.contains("PROJECTION_DIVERGED"));
    assert_eq!(fs::read(&path).unwrap(), b"changed by another writer\n");
    assert!(!backup.exists());
    let temp_residue = fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains(".config.toml.tmp-")
        });
    assert!(!temp_residue);
}
