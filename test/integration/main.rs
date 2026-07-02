use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use tempfile::tempdir;

fn megara() -> Command {
    Command::new(env!("CARGO_BIN_EXE_megara"))
}

fn megara_with_codex_home(codex_home: &Path) -> Command {
    let mut command = megara();
    command.env("CODEX_HOME", codex_home);
    command
}

fn run_hook(
    project_root: &Path,
    cwd: &Path,
    event: &str,
    matcher: Option<&str>,
    payload: &[u8],
) -> Output {
    let mut command = megara();
    command
        .arg("hook")
        .arg("--scope")
        .arg("project")
        .arg("--project-root")
        .arg(project_root)
        .arg("--runtime")
        .arg("codex")
        .arg("--event")
        .arg(event)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(matcher) = matcher {
        command.arg("--matcher").arg(matcher);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(payload).unwrap();
    child.wait_with_output().unwrap()
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

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

mod doctor;
mod hook_deep_interview;
mod hook_deep_interview_support;
mod hook_ralplan_approval;
mod hook_ralplan_direct;
mod hook_ralplan_handoff;
mod hook_ralplan_input_lock;
mod hook_ralplan_priority;
mod hook_ralplan_refine;
mod hook_ralplan_support;
mod hook_runtime;
mod hook_runtime_scope;
mod hook_runtime_ultragoal;
mod install;
mod install_global;
mod install_listing;
mod install_sync;
mod install_trust;
mod ultragoal;
mod ultragoal_support;
