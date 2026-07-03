use super::*;

#[test]
fn lists_targets_and_templates() {
    let targets = megara().arg("targets").arg("list").output().unwrap();
    assert!(targets.status.success());
    let target_stdout = String::from_utf8_lossy(&targets.stdout);
    assert!(target_stdout.contains("Megara / Targets"));
    assert!(target_stdout.contains("codex"));

    let templates = megara().arg("templates").arg("list").output().unwrap();
    assert!(templates.status.success());
    let stdout = String::from_utf8_lossy(&templates.stdout);
    assert!(stdout.contains("Megara / Templates"));
    assert!(stdout.contains("deep-interview"));
    assert!(stdout.contains("caveman"));
    assert!(stdout.contains("deep-interview/auto-research-greenfield"));
    assert!(!stdout.contains("megara-hook"));

    let update_help = megara().arg("update").arg("--help").output().unwrap();
    assert!(update_help.status.success());
    assert!(String::from_utf8_lossy(&update_help.stdout).contains("Update the Megara binary"));
}
