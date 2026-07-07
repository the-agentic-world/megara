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
    fs::write(dir.path().join("app.txt"), "changed\n").unwrap();

    let output = completion(dir.path(), "sess-uncommitted");
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""decision":"block""#));
    assert!(stdout.contains("commit required"));
    assert!(stdout.contains("app.txt"));
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
    assert!(stdout.contains(r#""decision":"block""#));
    assert!(stdout.contains("commit type"));
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
    assert!(stdout.contains(r#""decision":"block""#));
    assert!(stdout.contains("mixed path groups"));
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
    assert!(stdout.contains(r#""decision":"block""#));
    assert!(stdout.contains("overlap"));
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
