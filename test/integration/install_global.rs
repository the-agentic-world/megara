use super::*;

#[test]
fn installs_global_scope_codex_harness() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let codex_home = home.path().join(".codex");

    let output = megara_with_codex_home(&codex_home)
        .arg("install")
        .arg("--scope")
        .arg("global")
        .arg("--target")
        .arg("codex")
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.path().join(".megara/megara.toml").exists());
    assert!(home.path().join(".megara/bin/megara").exists());
    assert!(home.path().join(".megara/bin/insane-search").exists());
    assert!(home
        .path()
        .join(".megara/tools/insane-search/TOOL.md")
        .exists());
    assert!(home.path().join(".codex/AGENTS.md").exists());
    let agents_md = fs::read_to_string(home.path().join(".codex/AGENTS.md")).unwrap();
    assert!(agents_md.contains("~/.megara/bin/<tool-name>"));
}
