use super::*;

#[test]
fn project_hook_rejects_cwd_outside_project_root() {
    let project = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(project.path(), codex_home.path());

    let output = run_hook(
        project.path(),
        outside.path(),
        "UserPromptSubmit",
        None,
        br#"{"prompt":"outside"}"#,
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside project root"));
    assert!(!project
        .path()
        .join(".agents/state/hooks/events.jsonl")
        .exists());
    assert!(!outside
        .path()
        .join(".agents/state/hooks/events.jsonl")
        .exists());
}
