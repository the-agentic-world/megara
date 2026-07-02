use super::*;

#[test]
fn lists_targets_and_templates() {
    let targets = megara().arg("targets").arg("list").output().unwrap();
    assert!(targets.status.success());
    assert!(String::from_utf8_lossy(&targets.stdout).contains("codex"));

    let templates = megara().arg("templates").arg("list").output().unwrap();
    assert!(templates.status.success());
    let stdout = String::from_utf8_lossy(&templates.stdout);
    assert!(stdout.contains("deep-interview"));
    assert!(stdout.contains("deep-interview/auto-research-greenfield"));
    assert!(!stdout.contains("megara-hook"));
}
