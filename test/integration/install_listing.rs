use super::*;

#[test]
fn lists_targets_and_templates() {
    let targets = megara().arg("targets").arg("list").output().unwrap();
    assert!(targets.status.success());
    let target_stdout = String::from_utf8_lossy(&targets.stdout);
    assert!(target_stdout.contains("Megara / Targets"));
    assert!(target_stdout.contains("codex"));
    assert!(target_stdout.contains("pi"));

    let templates = megara().arg("templates").arg("list").output().unwrap();
    assert!(templates.status.success());
    let stdout = String::from_utf8_lossy(&templates.stdout);
    assert!(stdout.contains("Megara / Templates"));
    assert!(stdout.contains("deep-interview"));
    assert!(stdout.contains("caveman"));
    assert!(stdout.contains("insane-search"));
    assert_eq!(stdout.matches("insane-search").count(), 1);
    assert!(stdout.contains("deep-interview/auto-research-greenfield"));
    assert!(!stdout.contains("insane-search/engine/fetch_chain.py"));
    assert!(!stdout.contains("megara-hook"));

    let tool = megara()
        .arg("templates")
        .arg("show")
        .arg("insane-search")
        .output()
        .unwrap();
    assert!(tool.status.success());
    let skill_stdout = String::from_utf8_lossy(&tool.stdout);
    assert!(skill_stdout.contains("kind: skill"));
    assert!(skill_stdout.contains("name: insane-search"));
    assert!(skill_stdout.contains(".agents/tools/insane-search/TOOL.md"));

    let tool = megara()
        .arg("templates")
        .arg("show")
        .arg("tools/insane-search/TOOL.md")
        .output()
        .unwrap();
    assert!(tool.status.success());
    let tool_stdout = String::from_utf8_lossy(&tool.stdout);
    assert!(tool_stdout.contains("kind: tool"));
    assert!(tool_stdout.contains("https://github.com/fivetaku/insane-search"));

    let update_help = megara().arg("update").arg("--help").output().unwrap();
    assert!(update_help.status.success());
    assert!(String::from_utf8_lossy(&update_help.stdout).contains("Update the Megara binary"));
}
