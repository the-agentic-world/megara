use std::process::Command;

use tempfile::tempdir;

fn megara(project: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_megara"));
    command
        .env("MEGARA_NO_UPDATE_CHECK", "1")
        .current_dir(project);
    command
}

#[test]
fn removed_runtime_commands_are_rejected_without_side_effects() {
    let project = tempdir().unwrap();
    let removed_commands = [
        vec!["hook".to_string()],
        vec!["team".to_string()],
        vec!["ultra".to_string() + "goal"],
        vec!["pi".to_string(), "event".to_string()],
    ];

    for args in removed_commands {
        let output = megara(project.path()).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "command unexpectedly accepted: {args:?}"
        );
        assert!(!project.path().join(".megara").exists());
    }
}
