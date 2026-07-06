use super::hook_ralplan_support::read_json;
use super::ultragoal_support::*;
use super::*;

#[test]
fn ultragoal_cli_creates_goals_and_records_completion_receipt() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();
    install_project_harness(dir.path(), codex_home.path());

    let brief = "@goal: Board shell\nBuild the playable board shell.\n\n@goal Score model\nTrack scores and losses.";
    let direct = create_goals(dir.path(), true);
    assert!(!direct.status.success());
    assert!(String::from_utf8_lossy(&direct.stderr).contains("--allow-direct"));

    approve_ralplan_for_ultragoal(dir.path(), brief);
    let create = create_goals(dir.path(), false);
    assert!(
        create.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&create.stderr)
    );

    let state_dir = dir.path().join(".megara/artifacts/ultragoal/sess-ug");
    assert!(state_dir.join("brief.md").exists());
    let goals_path = state_dir.join("goals.json");
    let goals = read_json(&goals_path);
    assert_eq!(goals["source"]["kind"], "ralplan");
    assert_eq!(goals["source"]["ralplan_plan_id"], PLAN_ID);
    assert_eq!(goals["goals"][0]["id"], "G001");
    assert_eq!(goals["goals"][0]["title"], "Board shell");
    assert_eq!(goals["goals"][1]["status"], "pending");

    let runtime_state_path = dir
        .path()
        .join(".megara/state/workflows/ultragoal/sess-ug.json");
    assert_eq!(read_json(&runtime_state_path)["phase"], "goal_planning");

    let next = complete_goals(dir.path());
    assert!(
        next.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&next.stderr)
    );
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["state"], "started");
    assert_eq!(next["next_goal"]["id"], "G001");
    let runtime_state = read_json(&runtime_state_path);
    assert_eq!(runtime_state["phase"], "active");
    assert_eq!(runtime_state["active_goal_id"], "G001");

    let quality_gate = write_quality_artifacts(dir.path());
    let checkpoint = complete_checkpoint(dir.path(), &quality_gate);
    assert!(
        checkpoint.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&checkpoint.stderr)
    );
    let checkpoint: serde_json::Value = serde_json::from_slice(&checkpoint.stdout).unwrap();
    assert_eq!(checkpoint["goal"]["status"], "complete");
    assert_eq!(checkpoint["goal"]["completion_receipt"]["goal_id"], "G001");
    assert_eq!(checkpoint["next_goal_started"]["id"], "G002");

    let goals = read_json(&goals_path);
    assert_eq!(goals["goals"][0]["status"], "complete");
    assert_eq!(
        goals["goals"][0]["completion_receipt"]["receipt_id"]
            .as_str()
            .unwrap()
            .len(),
        19
    );
    assert_eq!(goals["goals"][1]["status"], "active");

    let status = ultragoal_status(dir.path());
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["counts"]["complete"], 1);
    assert_eq!(status["counts"]["active"], 1);
    assert_eq!(status["active_goal"]["id"], "G002");

    let ledger = fs::read_to_string(state_dir.join("ledger.jsonl")).unwrap();
    assert!(ledger.contains("\"event\":\"goals_created\""));
    assert!(ledger.contains("\"event\":\"goal_started\""));
    assert!(ledger.contains("\"event\":\"goal_checkpointed\""));
}
