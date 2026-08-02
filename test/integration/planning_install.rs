use super::*;

fn install(project: &Path, codex_home: &Path, extra: &[&str]) -> Output {
    let mut command = megara_with_codex_home(codex_home);
    command
        .args([
            "install",
            "--scope",
            "project",
            "--target",
            "codex",
            "--no-interactive",
        ])
        .args(extra)
        .current_dir(project);
    command.output().unwrap()
}

#[test]
fn project_install_merges_existing_codex_config_without_whole_file_marker() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let original = "# keep this comment\n[mcp_servers.other]\ncommand = \"other\"\n";
    fs::write(&config, original).unwrap();

    let output = install(project.path(), codex_home.path(), &[]);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let updated = fs::read_to_string(&config).unwrap();
    assert!(updated.starts_with(original));
    assert!(updated.contains("[mcp_servers.megara_planning]"));
    assert!(!updated.contains("MEGARA:MANAGED"));
    let parsed: toml::Value = toml::from_str(&updated).unwrap();
    assert_eq!(
        parsed["mcp_servers"]["other"]["command"].as_str(),
        Some("other")
    );
}

#[test]
fn unmanaged_mcp_table_conflict_has_zero_writes() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let original = "[mcp_servers.megara_planning]\ncommand = \"user-owned\"\n";
    fs::write(&config, original).unwrap();

    let output = install(project.path(), codex_home.path(), &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unmanaged"));
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
    assert!(!project.path().join(".codex/AGENTS.md").exists());
    assert!(!project
        .path()
        .join(".codex/config.toml.megara.mcp.bak")
        .exists());
}

#[test]
fn force_updates_only_the_mcp_table_and_keeps_exact_table_backup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let managed = "[mcp_servers.megara_planning]\n# preserve table comment\ncommand = \"old\"\n";
    let original = format!("# before\n{managed}[mcp_servers.other]\ncommand = \"other\"\n");
    fs::write(&config, &original).unwrap();

    let output = install(project.path(), codex_home.path(), &["--force"]);
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let backup = fs::read(config.with_file_name("config.toml.megara.mcp.bak")).unwrap();
    assert_eq!(backup, managed.as_bytes());
    let updated = fs::read_to_string(&config).unwrap();
    assert!(updated.contains("command = \"other\""));
    assert!(updated.contains("[mcp_servers.megara_planning]"));
    assert!(updated.contains("MEGARA:MCP-SHA256="));
    assert!(!updated.contains("# preserve table comment"));
}

#[test]
fn existing_backup_is_never_overwritten() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    let backup = config.with_file_name("config.toml.megara.mcp.bak");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "[mcp_servers.megara_planning]\ncommand = \"old\"\n",
    )
    .unwrap();
    fs::write(&backup, "do not overwrite\n").unwrap();

    let output = install(project.path(), codex_home.path(), &["--force"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("backup"));
    assert_eq!(fs::read_to_string(&backup).unwrap(), "do not overwrite\n");
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "[mcp_servers.megara_planning]\ncommand = \"old\"\n"
    );
}

#[test]
fn invalid_utf8_config_is_not_treated_as_absent() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, [0xff, 0xfe, 0x0a]).unwrap();

    let output = install(project.path(), codex_home.path(), &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    assert!(!project.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn dry_run_force_does_not_write_table_or_backup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let original = "[mcp_servers.megara_planning]\ncommand = \"old\"\n";
    fs::write(&config, original).unwrap();

    let output = install(project.path(), codex_home.path(), &["--force", "--dry-run"]);
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
    assert!(!config.with_file_name("config.toml.megara.mcp.bak").exists());
    assert!(!project.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn uninstall_removes_only_hash_matched_mcp_table() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "# keep\n[mcp_servers.other]\ncommand = \"other\"\n",
    )
    .unwrap();
    assert!(install(project.path(), codex_home.path(), &[])
        .status
        .success());

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    let remaining = fs::read_to_string(&config).unwrap();
    assert!(remaining.contains("# keep"));
    assert!(remaining.contains("command = \"other\""));
    assert!(!remaining.contains("megara_planning"));
    assert!(!remaining.contains("MEGARA:MCP-SHA256="));
}

#[test]
fn forced_uninstall_of_hash_matched_table_does_not_require_backup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    assert!(install(project.path(), codex_home.path(), &[])
        .status
        .success());
    let config = project.path().join(".codex/config.toml");
    let backup = config.with_file_name("config.toml.megara.mcp.bak");
    fs::write(&backup, "pre-existing backup\n").unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "uninstall",
            "--scope",
            "project",
            "--target",
            "codex",
            "--force",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert_eq!(
        fs::read_to_string(&backup).unwrap(),
        "pre-existing backup\n"
    );
    assert!(!fs::read_to_string(config)
        .unwrap()
        .contains("megara_planning"));
}

#[test]
fn uninstall_preserves_edited_table_without_force() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    assert!(install(project.path(), codex_home.path(), &[])
        .status
        .success());
    let config = project.path().join(".codex/config.toml");
    let edited = fs::read_to_string(&config)
        .unwrap()
        .replace("enabled = true", "enabled = false");
    fs::write(&config, &edited).unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args(["uninstall", "--scope", "project", "--target", "codex"])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("edited or unmanaged"));
    assert_eq!(fs::read_to_string(&config).unwrap(), edited);
    assert!(project.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn uninstall_config_failure_preserves_hook_trust_and_managed_files() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    assert!(install(project.path(), codex_home.path(), &[])
        .status
        .success());
    let config = project.path().join(".codex/config.toml");
    let edited = fs::read_to_string(&config)
        .unwrap()
        .replace("enabled = true", "enabled = false");
    fs::write(&config, edited.clone()).unwrap();
    let backup = config.with_file_name("config.toml.megara.mcp.bak");
    fs::write(&backup, "preserve this backup\n").unwrap();
    let codex_config = codex_home.path().join("config.toml");
    let trust_before = fs::read(&codex_config).unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .args([
            "uninstall",
            "--scope",
            "project",
            "--target",
            "codex",
            "--force",
        ])
        .current_dir(project.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(&codex_config).unwrap(), trust_before);
    assert_eq!(fs::read(&config).unwrap(), edited.as_bytes());
    assert_eq!(fs::read(&backup).unwrap(), b"preserve this backup\n");
    assert!(project.path().join(".codex/AGENTS.md").exists());
    assert!(project.path().join(".codex/hooks.json").exists());
}

#[test]
fn generic_preflight_failure_does_not_leave_mcp_backup() {
    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    assert!(install(project.path(), codex_home.path(), &[])
        .status
        .success());
    let config = project.path().join(".codex/config.toml");
    let edited = fs::read_to_string(&config)
        .unwrap()
        .replace("enabled = true", "enabled = false");
    fs::write(&config, edited).unwrap();
    fs::remove_file(project.path().join(".codex/AGENTS.md")).unwrap();
    fs::create_dir(project.path().join(".codex/AGENTS.md")).unwrap();

    let output = install(project.path(), codex_home.path(), &["--force"]);
    assert!(!output.status.success());
    assert!(!config.with_file_name("config.toml.megara.mcp.bak").exists());
    assert!(project.path().join(".codex/AGENTS.md").is_dir());
}

#[cfg(unix)]
#[test]
fn managed_config_update_preserves_existing_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let project = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    let config = project.path().join(".codex/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        "[mcp_servers.megara_planning]\ncommand = \"old\"\n",
    )
    .unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(install(project.path(), codex_home.path(), &["--force"])
        .status
        .success());
    assert_eq!(
        fs::metadata(config).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
