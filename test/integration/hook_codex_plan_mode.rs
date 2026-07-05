use super::*;

const THREAD_ID: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

#[test]
fn deep_interview_in_codex_plan_mode_passes_without_proxy() {
    let dir = tempdir().unwrap();
    let payload = payload("$deep-interview improve the menu", "plan");

    let output = run_hook(dir.path(), dir.path(), "UserPromptSubmit", None, &payload);

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn deep_interview_with_plan_prefix_blocks_when_proxy_missing() {
    let dir = tempdir().unwrap();
    let missing_proxy = dir.path().join("missing-proxy");
    let missing_proxy = missing_proxy.to_string_lossy().to_string();
    let payload = payload("/plan $deep-interview improve the menu", "default");

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[("MEGARA_CODEX_APP_SERVER_PROXY", missing_proxy.as_str())],
    );

    assert_blocked_for_plan_mode(&output);
}

#[test]
fn delegated_deep_interview_with_plan_prefix_blocks_when_proxy_missing() {
    let dir = tempdir().unwrap();
    let missing_proxy = dir.path().join("missing-proxy");
    let missing_proxy = missing_proxy.to_string_lossy().to_string();
    let payload = payload(
        "<codex_delegation><input>/plan $deep-interview improve the menu</input></codex_delegation>",
        "default",
    );

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[("MEGARA_CODEX_APP_SERVER_PROXY", missing_proxy.as_str())],
    );

    assert_blocked_for_plan_mode(&output);
}

#[test]
fn deep_interview_with_transcript_plan_mode_passes_without_proxy() {
    let dir = tempdir().unwrap();
    let missing_proxy = dir.path().join("missing-proxy");
    let missing_proxy = missing_proxy.to_string_lossy().to_string();
    let transcript = dir.path().join("session.jsonl");
    fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-plan","collaboration_mode_kind":"plan"}}
{"type":"turn_context","payload":{"turn_id":"turn-plan","collaboration_mode":{"mode":"plan","settings":{"model":"gpt-5.5"}}}}"#,
    )
    .unwrap();
    let payload = payload_with_transcript(
        "$deep-interview improve the menu",
        "bypassPermissions",
        "turn-plan",
        &transcript,
    );

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[("MEGARA_CODEX_APP_SERVER_PROXY", missing_proxy.as_str())],
    );

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn deep_interview_blocks_when_proxy_missing() {
    let dir = tempdir().unwrap();
    let missing_proxy = dir.path().join("missing-proxy");
    let missing_proxy = missing_proxy.to_string_lossy().to_string();
    let payload = payload("$deep-interview improve the menu", "default");

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[("MEGARA_CODEX_APP_SERVER_PROXY", missing_proxy.as_str())],
    );

    assert_blocked_for_plan_mode(&output);
}

#[test]
fn delegated_deep_interview_blocks_when_proxy_missing() {
    let dir = tempdir().unwrap();
    let missing_proxy = dir.path().join("missing-proxy");
    let missing_proxy = missing_proxy.to_string_lossy().to_string();
    let payload = payload(
        "<codex_delegation><input>$deep-interview improve the menu</input></codex_delegation>",
        "default",
    );

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[("MEGARA_CODEX_APP_SERVER_PROXY", missing_proxy.as_str())],
    );

    assert_blocked_for_plan_mode(&output);
}

#[test]
fn deep_interview_activates_with_fake_proxy() {
    let dir = tempdir().unwrap();
    let (_proxy_dir, proxy) = compile_fake_proxy();
    let proxy = proxy.to_string_lossy().to_string();
    let log = dir.path().join("fake-proxy.log");
    let log = log.to_string_lossy().to_string();
    let payload = payload("$deep-interview improve the menu", "default");

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[
            ("MEGARA_CODEX_APP_SERVER_PROXY", proxy.as_str()),
            ("MEGARA_FAKE_PROXY_MODE", "success"),
            ("MEGARA_FAKE_THREAD_ID", THREAD_ID),
            ("MEGARA_FAKE_PROXY_LOG", log.as_str()),
        ],
    );

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains("\"method\":\"thread/settings/update\""));
    assert!(log.contains(THREAD_ID));
    assert!(log.contains("\"mode\":\"plan\""));
}

#[test]
fn deep_interview_blocks_when_experimental_api_unsupported() {
    let dir = tempdir().unwrap();
    let (_proxy_dir, proxy) = compile_fake_proxy();
    let proxy = proxy.to_string_lossy().to_string();
    let payload = payload("$deep-interview improve the menu", "default");

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[
            ("MEGARA_CODEX_APP_SERVER_PROXY", proxy.as_str()),
            ("MEGARA_FAKE_PROXY_MODE", "unsupported"),
        ],
    );

    assert_blocked_for_plan_mode(&output);
}

#[test]
fn deep_interview_blocks_when_update_notification_missing() {
    let dir = tempdir().unwrap();
    let (_proxy_dir, proxy) = compile_fake_proxy();
    let proxy = proxy.to_string_lossy().to_string();
    let payload = payload("$deep-interview improve the menu", "default");

    let output = run_hook_with_env(
        dir.path(),
        dir.path(),
        "UserPromptSubmit",
        None,
        &payload,
        &[
            ("MEGARA_CODEX_APP_SERVER_PROXY", proxy.as_str()),
            ("MEGARA_FAKE_PROXY_MODE", "timeout"),
            ("MEGARA_CODEX_PLAN_MODE_TIMEOUT_MS", "80"),
        ],
    );

    assert_blocked_for_plan_mode(&output);
}

fn payload(prompt: &str, permission_mode: &str) -> Vec<u8> {
    let transcript_path = PathBuf::from(format!(
        "/Users/me/.codex/sessions/2026/01/01/rollout-2026-01-01T00-00-00-{THREAD_ID}.jsonl"
    ));
    payload_with_transcript(prompt, permission_mode, "turn-1", &transcript_path)
}

fn payload_with_transcript(
    prompt: &str,
    permission_mode: &str,
    turn_id: &str,
    transcript_path: &Path,
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "prompt": prompt,
        "permission_mode": permission_mode,
        "turn_id": turn_id,
        "model": "gpt-5.5",
        "session_id": "session-alias",
        "transcript_path": transcript_path,
    }))
    .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_blocked_for_plan_mode(output: &Output) {
    assert_success(output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""decision":"block""#), "stdout={stdout}");
    assert!(stdout.contains("Codex Plan mode"), "stdout={stdout}");
    assert!(stdout.contains("Plan mode"), "stdout={stdout}");
    assert!(!stdout.contains("/plan <same request>"), "stdout={stdout}");
    assert!(!stdout.contains("app-server"), "stdout={stdout}");
    assert!(!stdout.contains("thread"), "stdout={stdout}");
}

fn compile_fake_proxy() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let source = dir.path().join("fake_proxy.rs");
    let binary = dir
        .path()
        .join(format!("fake-proxy{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, FAKE_PROXY_SOURCE).unwrap();
    let output = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (dir, binary)
}

const FAKE_PROXY_SOURCE: &str = r##"
use std::{
    env,
    fs::OpenOptions,
    io::{self, BufRead, Write},
    thread,
    time::Duration,
};

fn main() {
    let mode = env::var("MEGARA_FAKE_PROXY_MODE").unwrap_or_else(|_| "success".to_string());
    let thread_id = env::var("MEGARA_FAKE_THREAD_ID")
        .unwrap_or_else(|_| "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string());
    let log_path = env::var("MEGARA_FAKE_PROXY_LOG").ok();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        if let Some(path) = &log_path {
            let mut log = OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(log, "{line}").unwrap();
        }

        if line.contains("\"method\":\"initialize\"") {
            stdout.write_all(br#"{"id":1,"result":{}}"#).unwrap();
            stdout.write_all(b"\n").unwrap();
            stdout.flush().unwrap();
        } else if line.contains("\"method\":\"collaborationMode/list\"") {
            if mode == "unsupported" {
                stdout
                    .write_all(br#"{"id":2,"error":{"code":-32601,"message":"method not found"}}"#)
                    .unwrap();
                stdout.write_all(b"\n").unwrap();
            } else {
                stdout
                    .write_all(br#"{"id":2,"result":{"data":[{"name":"Default","mode":"default","model":"gpt-5.5","reasoning_effort":"medium"},{"name":"Plan","mode":"plan","model":"gpt-5.5","reasoning_effort":"medium"}]}}"#)
                    .unwrap();
                stdout.write_all(b"\n").unwrap();
            }
            stdout.flush().unwrap();
        } else if line.contains("\"method\":\"thread/settings/update\"") {
            stdout.write_all(br#"{"id":3,"result":{}}"#).unwrap();
            stdout.write_all(b"\n").unwrap();
            if mode == "timeout" {
                thread::sleep(Duration::from_millis(300));
            } else {
                writeln!(
                    stdout,
                    "{{\"method\":\"thread/settings/updated\",\"params\":{{\"threadId\":\"{}\",\"threadSettings\":{{\"collaborationMode\":{{\"mode\":\"plan\",\"settings\":{{\"model\":\"gpt-5.5\",\"reasoning_effort\":\"medium\",\"developer_instructions\":null}}}}}}}}}}",
                    thread_id
                )
                .unwrap();
            }
            stdout.flush().unwrap();
        }
    }
}
"##;
