use super::*;

#[test]
fn git_guard_allows_non_git_projects() {
    let dir = tempdir().unwrap();
    baseline_prompt(dir.path(), "sess-non-git");
    fs::write(dir.path().join("app.txt"), "changed\n").unwrap();

    let output = completion(dir.path(), "sess-non-git");
    assert_success(&output);
}

#[test]
fn git_guard_blocks_uncommitted_agent_delta() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-uncommitted");
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/2048-evidence.md"), "changed\n").unwrap();

    let output = completion(dir.path(), "sess-uncommitted");
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("MEGARA git guard"));
    assert!(!stdout.contains("Stage explicit files"));
    assert!(!stdout.contains("docs/2048-evidence.md"));

    let events = git_guard_events(dir.path());
    assert!(events.contains("uncommitted agent changes: docs/2048-evidence.md"));
    assert!(!events.contains(", ocs/2048-evidence.md"));
}

#[test]
fn git_guard_ignores_preexisting_dirty_state() {
    let dir = git_project();
    fs::write(dir.path().join("app.txt"), "preexisting\n").unwrap();
    baseline_prompt(dir.path(), "sess-dirty");

    let output = completion(dir.path(), "sess-dirty");
    assert_success(&output);
}

#[test]
fn git_guard_allows_valid_focused_commit() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-valid");
    fs::write(dir.path().join("app.txt"), "changed\n").unwrap();
    git(dir.path(), &["add", "app.txt"]);
    git(dir.path(), &["commit", "-m", "fix: update app behavior"]);

    let output = completion(dir.path(), "sess-valid");
    assert_success(&output);
}

#[test]
fn git_guard_blocks_invalid_commit_message() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-invalid-message");
    fs::write(dir.path().join("app.txt"), "changed\n").unwrap();
    git(dir.path(), &["add", "app.txt"]);
    git(dir.path(), &["commit", "-m", "Fix: Updated app behavior."]);

    let output = completion(dir.path(), "sess-invalid-message");
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("commit type"));

    let events = git_guard_events(dir.path());
    assert!(events.contains("commit type"));
}

#[test]
fn git_guard_blocks_mixed_intent_commit() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-mixed");
    for path in [
        "src/app.rs",
        "src/lib.rs",
        "docs/guide.md",
        "tests/app.rs",
        "harness/rules.md",
        "Cargo.toml",
    ] {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, "changed\n").unwrap();
        git(dir.path(), &["add", path]);
    }
    git(
        dir.path(),
        &["commit", "-m", "feat: add broad project changes"],
    );

    let output = completion(dir.path(), "sess-mixed");
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("mixed path groups"));

    let events = git_guard_events(dir.path());
    assert!(events.contains("mixed path groups"));
}

#[test]
fn git_guard_blocks_unsafe_git_add_dot() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-add-dot");

    let output = run_hook(
        dir.path(),
        dir.path(),
        "PreToolUse",
        Some("Bash"),
        br#"{"session_id":"sess-add-dot","tool_input":{"command":"git add . && git commit -m 'fix: update app behavior'"}}"#,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("do not use `git add .`"));
}

#[test]
fn git_guard_blocks_overlap_with_preexisting_dirty_path() {
    let dir = git_project();
    fs::write(dir.path().join("app.txt"), "preexisting\n").unwrap();
    baseline_prompt(dir.path(), "sess-overlap");
    fs::write(dir.path().join("app.txt"), "agent mixed with preexisting\n").unwrap();
    git(dir.path(), &["add", "app.txt"]);
    git(dir.path(), &["commit", "-m", "fix: update app behavior"]);

    let output = completion(dir.path(), "sess-overlap");
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("overlap"));

    let events = git_guard_events(dir.path());
    assert!(events.contains("overlap"));
}

#[test]
fn git_guard_feedback_leak_is_blocked_before_user_output() {
    let dir = git_project();
    baseline_prompt(dir.path(), "sess-leak");

    let payload = format!(
        r#"{{"session_id":"sess-leak","cwd":"{}","last_assistant_message":"<hook_prompt hook_run_id=\"stop:5:/tmp/hooks.json\">MEGARA git guard: commit required before completion. Stage explicit files only (`git add <file>`), create focused OMA /scm-style Conventional Commits, rerun verification, then answer again.</hook_prompt>\n\n완료했습니다."}}"#,
        dir.path().display()
    );
    let output = run_hook(dir.path(), dir.path(), "Stop", None, payload.as_bytes());
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty());
    assert!(!stdout.contains("MEGARA git guard"));
    assert!(!stdout.contains("Stage explicit files"));
    assert!(!stdout.contains("<hook_prompt"));
    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl"))
            .unwrap_or_default();
    assert!(!conversation.contains("MEGARA git guard"));
    assert!(!conversation.contains("<hook_prompt"));
}

#[test]
fn git_guard_ignores_internal_hook_feedback_as_user_prompt() {
    let dir = git_project();
    let payload = format!(
        r#"{{"session_id":"sess-hook-feedback","cwd":"{}","prompt":"<hook_prompt hook_run_id=\"stop:5:/tmp/hooks.json\">Megara needs an internal git cleanup pass before the final response.</hook_prompt>"}}"#,
        dir.path().display()
    );

    let output = run_hook(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    );

    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(!dir
        .path()
        .join(".megara/state/hooks/git-guard/sess-hook-feedback.json")
        .exists());
    let conversation =
        fs::read_to_string(dir.path().join(".megara/state/hooks/conversation.jsonl"))
            .unwrap_or_default();
    assert!(!conversation.contains("internal git cleanup pass"));
    assert!(!conversation.contains("<hook_prompt"));
}

fn git_project() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Megara Test"]);
    git(dir.path(), &["config", "user.email", "megara@example.test"]);
    fs::write(dir.path().join("app.txt"), "initial\n").unwrap();
    git(dir.path(), &["add", "app.txt"]);
    git(dir.path(), &["commit", "-m", "chore: initial commit"]);
    dir
}

fn git(project: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}\nstdout={}\nstderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn baseline_prompt(project: &Path, session_id: &str) {
    let payload = format!(
        r#"{{"session_id":"{session_id}","cwd":"{}","prompt":"implement the requested change"}}"#,
        project.display()
    );
    let output = run_hook(
        project,
        project,
        "UserPromptSubmit",
        None,
        payload.as_bytes(),
    );
    assert_success(&output);
}

fn completion(project: &Path, session_id: &str) -> Output {
    let payload = format!(
        r#"{{"session_id":"{session_id}","cwd":"{}","last_assistant_message":"구현 완료. 검증 통과."}}"#,
        project.display()
    );
    run_hook(project, project, "Stop", None, payload.as_bytes())
}

fn git_guard_events(project: &Path) -> String {
    fs::read_to_string(project.join(".megara/state/hooks/git-guard/events.jsonl")).unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
