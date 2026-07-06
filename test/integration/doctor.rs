use super::*;

#[test]
fn doctor_reports_missing_then_ok() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let missing = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(missing.status.success());
    let missing_stdout = String::from_utf8_lossy(&missing.stdout);
    assert!(missing_stdout.contains("\"ok\": false"));

    let install = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success());

    let agents_md = dir.path().join(".codex/AGENTS.md");
    fs::write(&agents_md, "# MEGARA:MANAGED\nstale").unwrap();

    let stale = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(stale.status.success());
    let stale_stdout = String::from_utf8_lossy(&stale.stdout);
    assert!(stale_stdout.contains("\"ok\": false"));
    assert!(stale_stdout.contains(".codex/AGENTS.md"));

    let sync = megara_with_codex_home(codex_home.path())
        .arg("sync")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(sync.status.success());

    let ok = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(ok.status.success());
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("\"ok\": true"));
    assert!(ok_stdout.contains("\"warnings\": []"));
    assert!(ok_stdout.contains("Codex hook events have not been observed yet"));

    let human = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(human.status.success());
    let human_stdout = String::from_utf8_lossy(&human.stdout);
    assert!(human_stdout.contains("Megara / Doctor"));
    assert!(human_stdout.contains("megara doctor: scope=project, target=codex, ok=true"));
}

#[test]
fn doctor_reports_broken_project_wrapper() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    fs::write(dir.path().join(".agents/bin/megara"), "not executable").unwrap();

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": false"));
    assert!(stdout.contains(".agents/bin/megara"));
}

#[test]
fn doctor_warns_about_legacy_agents_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    fs::create_dir_all(dir.path().join(".agents/state/workflows/deep-interview")).unwrap();

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": false"));
    assert!(stdout.contains("legacy Megara runtime state found under"));
    assert!(stdout.contains(".agents/state"));
    assert!(stdout.contains(".megara/state"));
}

#[test]
fn doctor_reports_stale_deep_interview_state() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let workflow_dir = dir.path().join(".megara/state/workflows/deep-interview");
    fs::create_dir_all(&workflow_dir).unwrap();
    fs::write(
        workflow_dir.join("ghost.json"),
        serde_json::json!({
            "version": 1,
            "skill": "deep-interview",
            "session_id": "ghost",
            "cwd": dir.path().display().to_string(),
            "active": true,
            "phase": "question_pending",
            "pending_question": {"id": "di-ghost", "status": "pending"},
            "questions": [],
            "updated_at": "1"
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        workflow_dir.join("visible.json"),
        serde_json::json!({
            "version": 1,
            "skill": "deep-interview",
            "session_id": "visible",
            "cwd": dir.path().display().to_string(),
            "active": false,
            "phase": "crystallized",
            "status": "crystallized",
            "pending_question": null,
            "questions": [],
            "updated_at": "2"
        })
        .to_string(),
    )
    .unwrap();

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": false"));
    assert!(stdout.contains("stale deep-interview state"));
    assert!(stdout.contains("ghost.json"));
}

#[test]
fn doctor_ignores_deep_interview_artifact_directories() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let workflow_dir = dir.path().join(".megara/state/workflows/deep-interview");
    let specs_dir = workflow_dir.join("specs");
    fs::create_dir_all(&specs_dir).unwrap();
    fs::write(specs_dir.join("index.jsonl"), "{}\n").unwrap();
    fs::write(specs_dir.join("spec.md"), "# Spec\n").unwrap();
    fs::write(
        workflow_dir.join("visible.json"),
        serde_json::json!({
            "version": 1,
            "skill": "deep-interview",
            "session_id": "visible",
            "cwd": dir.path().display().to_string(),
            "active": false,
            "phase": "crystallized",
            "status": "crystallized",
            "pending_question": null,
            "questions": [],
            "updated_at": "2"
        })
        .to_string(),
    )
    .unwrap();

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": true"));
}

#[test]
fn doctor_reports_duplicate_active_deep_interview_states() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let workflow_dir = dir.path().join(".megara/state/workflows/deep-interview");
    fs::create_dir_all(&workflow_dir).unwrap();
    for session_id in ["runtime-session", "visible-thread"] {
        fs::write(
            workflow_dir.join(format!("{session_id}.json")),
            serde_json::json!({
                "version": 1,
                "skill": "deep-interview",
                "session_id": session_id,
                "cwd": dir.path().display().to_string(),
                "active": true,
                "phase": "question_pending",
                "pending_question": {"id": "di-shared", "status": "pending"},
                "questions": [],
                "updated_at": "1"
            })
            .to_string(),
        )
        .unwrap();
    }

    let output = megara()
        .arg("doctor")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .arg("--json")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\": false"));
    assert!(stdout.contains("duplicate active deep-interview states"));
    assert!(stdout.contains("runtime-session.json"));
    assert!(stdout.contains("visible-thread.json"));
}
