use super::*;

#[test]
fn append_jsonl_keeps_concurrent_records_valid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let threads = (0..16)
        .map(|thread_id| {
            let path = path.clone();
            std::thread::spawn(move || {
                for record_id in 0..50 {
                    append_jsonl(
                        &path,
                        &json!({
                            "thread": thread_id,
                            "record": record_id,
                            "event": "PreToolUse",
                        }),
                    )
                    .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread.join().unwrap();
    }

    let contents = fs::read_to_string(&path).unwrap();
    let lines = contents.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 800);
    for line in lines {
        serde_json::from_str::<Value>(line).unwrap();
    }
}

#[test]
fn mutation_guard_allows_read_commands_with_discarded_stderr() {
    assert!(!mutating_command(
        "find .agents/state/workflows/deep-interview/specs -maxdepth 1 -type f -print 2>/dev/null | sort"
    ));
    assert!(!mutating_command(
        "tail -20 .agents/state/workflows/deep-interview/specs/index.jsonl 2>/dev/null"
    ));
    assert!(!mutating_command("grep needle file 2>&1"));
    assert!(!mutating_command("cat file > /dev/null"));
}

#[test]
fn mutation_guard_blocks_file_redirection() {
    assert!(mutating_command("echo hello > output.txt"));
    assert!(mutating_command("printf hello >> output.txt"));
    assert!(mutating_command("command 2> error.log"));
}

#[test]
fn approval_gate_ignores_delegation_closing_tag() {
    let text = "Megara Approval Gate:\n- plan_id: rp-dashboard-menu\n- plan_sha256: b3e252bef44736571b1d6aeeddf6105aef3d357ca1089d443d52fd188c738984\n- handoff_target: ultragoal</input>\n</codex_delegation>\n";

    let gate = approval_gate_from_text(text).unwrap();

    assert_eq!(gate.plan_id, "rp-dashboard-menu");
    assert_eq!(
        gate.plan_sha256,
        "b3e252bef44736571b1d6aeeddf6105aef3d357ca1089d443d52fd188c738984"
    );
    assert_eq!(gate.handoff_target, "ultragoal");
}

#[test]
fn plan_body_extraction_preserves_marker_mentions() {
    let text = "**Pending Execution Plan**\n\nMention this literal marker in the plan body:\n\nMegara Plan Gate:\nThis is documentation text, not the control block.\n\nContinue the actual plan here.\n\nMegara Plan Gate:\n- id: rp-marker-test\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-marker-test\n- next: approval\n";

    let body = text_before_first_workflow_block(text);

    assert!(body.contains("Megara Plan Gate:"));
    assert!(body.contains("This is documentation text"));
    assert!(body.contains("Continue the actual plan here."));
    assert!(!body.contains("- id: rp-marker-test"));
    assert!(!body.contains("Megara Workflow State:"));
}

#[test]
fn plan_body_extraction_hides_metadata_comment() {
    let text = "**Pending Execution Plan**\n\nSummary: add a dashboard.\n\nAcceptance criteria:\n- Existing flow still works.\n\n<!--\nMegara Plan Gate:\n- id: rp-dashboard\n- status: pending_approval\n- question: Approve this plan?\n- options:\n  - refine\n  - approve_ultragoal\n- free_text: false\n\nMegara Workflow State:\n- skill: ralplan\n- status: pending_approval\n- plan_id: rp-dashboard\n- next: approval\n-->\n";

    let body = text_before_first_workflow_block(text);

    assert!(body.contains("Summary: add a dashboard."));
    assert!(!body.contains("<!--"));
    assert!(!body.contains("Megara Plan Gate:"));
    assert!(!body.contains("Megara Workflow State:"));
}

#[test]
fn block_parser_does_not_steal_fields_after_prose() {
    let text = "Megara Plan Gate:\nThis marker is only discussed in prose.\n\nMegara Plan Gate:\n- id: rp-real\n- status: pending_approval\n";

    let blocks = parse_blocks(text, "Megara Plan Gate:");

    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0].fields.get("id").map(String::as_str),
        Some("rp-real")
    );
}
