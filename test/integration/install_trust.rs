use super::*;

#[test]
fn install_registers_codex_hook_trust_state_once() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let install = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
    assert!(String::from_utf8_lossy(&install.stdout).contains("hook trust: registered=7"));

    let config_path = codex_home.path().join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    let hooks_path = fs::canonicalize(dir.path().join(".codex/hooks.json")).unwrap();
    let hooks_path = hooks_path.display().to_string();
    for event in [
        "pre_tool_use",
        "post_tool_use",
        "session_start",
        "stop",
        "subagent_start",
        "subagent_stop",
        "user_prompt_submit",
    ] {
        let header = format!("[hooks.state.\"{hooks_path}:{event}:0:0\"]");
        assert_eq!(occurrences(&config, &header), 1);
    }
    assert_eq!(occurrences(&config, "trusted_hash = \"sha256:"), 7);

    let sync = megara_with_codex_home(codex_home.path())
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(sync.status.success());
    assert!(String::from_utf8_lossy(&sync.stdout).contains("hook trust: registered=0"));
    assert_eq!(fs::read_to_string(config_path).unwrap(), config);
}
