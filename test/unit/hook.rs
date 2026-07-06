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
        "find .megara/artifacts/deep-interview/specs -maxdepth 1 -type f -print 2>/dev/null | sort"
    ));
    assert!(!mutating_command(
        "tail -20 .megara/artifacts/deep-interview/specs/index.jsonl 2>/dev/null"
    ));
    assert!(!mutating_command("grep needle file 2>&1"));
    assert!(!mutating_command("cat file > /dev/null"));
}

#[test]
fn workflow_paths_separate_state_and_artifacts() {
    let state_dir = Path::new("/tmp/megara-project/.megara/state/hooks");
    let paths = workflow_paths(
        state_dir,
        &json!({
            "session_id": "sess-structure"
        }),
        "deep-interview",
    );

    assert_eq!(paths.session_id, "sess-structure");
    assert_eq!(
        paths.workflow_dir,
        Path::new("/tmp/megara-project/.megara/state/workflows/deep-interview")
    );
    assert_eq!(
        paths.session_file,
        Path::new("/tmp/megara-project/.megara/state/workflows/deep-interview/sess-structure.json")
    );
    assert_eq!(
        paths.artifact_dir,
        Path::new("/tmp/megara-project/.megara/artifacts/deep-interview")
    );
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

#[test]
fn codex_plan_mode_detects_deep_interview_start_only() {
    assert!(is_deep_interview_start_prompt(
        "$deep-interview improve the menu"
    ));
    assert!(is_deep_interview_start_prompt(
        "[$deep-interview](/tmp/SKILL.md) improve the menu"
    ));
    assert!(is_deep_interview_start_prompt(
        "/plan $deep-interview improve the menu"
    ));
    assert!(is_deep_interview_start_prompt(
        "/plan[$deep-interview](/tmp/SKILL.md) improve the menu"
    ));
    assert!(!is_deep_interview_start_prompt(
        "please use deep-interview later"
    ));
}

#[test]
fn effective_prompt_extracts_codex_delegated_input() {
    let prompt =
        "<codex_delegation><input>$deep-interview improve the menu</input></codex_delegation>";

    let effective = effective_prompt_text(prompt);

    assert_eq!(effective, "$deep-interview improve the menu");
    assert!(is_deep_interview_start_prompt(&effective));
}

#[test]
fn effective_prompt_extracts_plan_prefix_from_delegated_input() {
    let prompt = "<codex_delegation>\n<input>\n/plan [$deep-interview](/tmp/SKILL.md) improve the menu\n</input>\n</codex_delegation>";

    let effective = effective_prompt_text(prompt);

    assert_eq!(
        effective,
        "/plan [$deep-interview](/tmp/SKILL.md) improve the menu"
    );
    assert!(is_deep_interview_start_prompt(&effective));
}

#[test]
fn runtime_context_reads_transcript_surface() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"exec","thread_source":"user","originator":"Codex CLI"}}"#,
    )
    .unwrap();
    let payload = json!({
        "prompt": "hello",
        "transcript_path": transcript,
    });

    let context = runtime_context(&payload);

    assert_eq!(context.surface, RuntimeSurface::Cli);
    assert_eq!(context.transcript_source.as_deref(), Some("exec"));
    assert_eq!(context.transcript_thread_source.as_deref(), Some("user"));
    assert_eq!(context.transcript_originator.as_deref(), Some("Codex CLI"));
}

#[test]
fn runtime_context_treats_vscode_transcript_as_app_surface() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"session_meta","payload":{"source":"vscode","thread_source":"subagent","originator":"Codex Desktop"}}"#,
    )
    .unwrap();
    let payload = json!({
        "prompt": "<codex_delegation><input>hello</input></codex_delegation>",
        "transcript_path": transcript,
    });

    let context = runtime_context(&payload);

    assert_eq!(context.surface, RuntimeSurface::App);
    assert_eq!(context.effective_prompt.as_deref(), Some("hello"));
    assert_eq!(context.transcript_source.as_deref(), Some("vscode"));
}

#[test]
fn assistant_message_falls_back_to_current_turn_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"turn_context","payload":{"turn_id":"old-turn"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"old answer"}]}}
{"type":"turn_context","payload":{"turn_id":"turn-plan"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","phase":"final","content":[{"type":"output_text","text":"current "},{"type":"output_text","text":"answer"}]}}"#,
    )
    .unwrap();
    let payload = json!({
        "turn_id": "turn-plan",
        "transcript_path": transcript,
    });

    assert_eq!(
        assistant_message_from_payload(&payload).as_deref(),
        Some("current answer")
    );
}

#[test]
fn assistant_message_does_not_use_stale_transcript_turn() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"turn_context","payload":{"turn_id":"old-turn"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"old answer"}]}}"#,
    )
    .unwrap();
    let payload = json!({
        "turn_id": "new-turn",
        "transcript_path": transcript,
    });

    assert_eq!(assistant_message_from_payload(&payload), None);
}

#[test]
fn assistant_message_prefers_payload_over_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"turn_context","payload":{"turn_id":"turn-plan"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"transcript answer"}]}}"#,
    )
    .unwrap();
    let payload = json!({
        "turn_id": "turn-plan",
        "transcript_path": transcript,
        "last_assistant_message": "payload answer",
    });

    assert_eq!(
        assistant_message_from_payload(&payload).as_deref(),
        Some("payload answer")
    );
}

#[test]
fn codex_plan_mode_reads_payload_collaboration_mode() {
    assert!(payload_reports_plan_mode(&json!({
        "collaboration_mode": {
            "mode": "plan"
        }
    })));
    assert!(payload_reports_plan_mode(&json!({
        "collaborationMode": {
            "mode": "Plan"
        }
    })));
    assert!(payload_reports_plan_mode(&json!({
        "collaboration_mode_kind": "plan"
    })));
    assert!(!payload_reports_plan_mode(&json!({
        "permission_mode": "bypassPermissions",
        "collaboration_mode": {
            "mode": "default"
        }
    })));
}

#[test]
fn codex_plan_mode_reads_transcript_current_turn_mode() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-plan","collaboration_mode_kind":"plan"}}"#,
    )
    .unwrap();
    let payload = json!({
        "turn_id": "turn-plan",
        "transcript_path": transcript,
        "permission_mode": "bypassPermissions"
    });

    assert!(payload_reports_plan_mode(&payload));
}

#[test]
fn codex_plan_mode_ignores_transcript_stale_plan_turn() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"task_started","turn_id":"old-turn","collaboration_mode_kind":"plan"}}"#,
    )
    .unwrap();
    let payload = json!({
        "turn_id": "new-turn",
        "transcript_path": transcript,
        "permission_mode": "bypassPermissions"
    });

    assert!(!payload_reports_plan_mode(&payload));
}

#[test]
fn codex_plan_mode_canonicalizes_thread_id() {
    let payload = json!({
        "thread_id": "thread-main",
        "session_id": "session-alias",
        "transcript_path": "/Users/me/.codex/sessions/rollout-2026-01-01T00-00-00-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl"
    });
    assert_eq!(
        thread_id_from_payload(&payload).as_deref(),
        Some("thread-main")
    );

    let payload = json!({
        "session_id": "session-alias",
        "transcript_path": "/Users/me/.codex/sessions/rollout-2026-01-01T00-00-00-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl"
    });
    assert_eq!(
        thread_id_from_payload(&payload).as_deref(),
        Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
    );

    let payload = json!({
        "session_id": "session-only",
    });
    assert_eq!(
        thread_id_from_payload(&payload).as_deref(),
        Some("session-only")
    );
}

#[test]
fn codex_plan_mode_builds_plan_update_payload() {
    let list_result = json!({
        "data": [
            {
                "name": "Default",
                "mode": "default",
                "settings": {
                    "model": "gpt-5.5",
                    "reasoning_effort": "medium"
                }
            },
            {
                "name": "Plan",
                "mode": "plan",
                "model": "gpt-5.5-codex",
                "reasoning_effort": "high"
            }
        ]
    });

    let collaboration_mode = plan_collaboration_mode(&list_result, Some("gpt-5.5")).unwrap();
    assert_eq!(collaboration_mode["mode"], "plan");
    assert_eq!(collaboration_mode["settings"]["model"], "gpt-5.5-codex");
    assert_eq!(collaboration_mode["settings"]["reasoning_effort"], "high");
    assert!(collaboration_mode["settings"]["developer_instructions"].is_null());

    let payload = thread_settings_update_payload("thread-1", collaboration_mode);
    assert_eq!(payload["threadId"], "thread-1");
    assert_eq!(payload["collaborationMode"]["mode"], "plan");
}

#[test]
fn codex_plan_mode_recognizes_update_notification() {
    let notification = json!({
        "method": "thread/settings/updated",
        "params": {
            "threadId": "thread-1",
            "threadSettings": {
                "collaborationMode": {
                    "mode": "plan"
                }
            }
        }
    });

    assert!(is_plan_settings_notification(&notification, "thread-1"));
    assert!(!is_plan_settings_notification(&notification, "thread-2"));
}
