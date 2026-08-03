use std::{fs, path::Path, process::Command};

use tempfile::tempdir;

fn megara() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_megara"));
    command.env("MEGARA_NO_UPDATE_CHECK", "1");
    command
}

fn megara_with_codex_home(codex_home: &Path) -> Command {
    let mut command = megara();
    command.env("CODEX_HOME", codex_home);
    command
}

fn install_project_harness(project: &Path, codex_home: &Path) {
    let install = megara_with_codex_home(codex_home)
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(project)
        .output()
        .unwrap();
    assert!(
        install.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );
}

mod agent_policies;
mod docs;
mod doctor;
mod install;
mod install_global;
mod install_listing;
mod install_sync;
mod legacy_cli_removal;
mod pi;
#[allow(dead_code, unused_imports)]
#[path = "../../src/planning.rs"]
mod planning;
mod planning_adapter_equivalence;
mod planning_cli;
mod planning_cli_aliases;
mod planning_cli_artifact_support;
mod planning_cli_artifacts;
mod planning_cli_evidence;
mod planning_install;
mod planning_mcp;
mod planning_migration;
mod planning_migration_concurrency;
mod planning_migration_races;
mod planning_migration_resources;
mod planning_migration_rollback;
mod planning_migration_safety;
mod planning_migration_support;
mod planning_pi;
mod uninstall;
mod update;
