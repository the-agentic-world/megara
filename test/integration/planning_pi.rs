use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{json, Value};
use tempfile::tempdir;

struct PiRpc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl PiRpc {
    fn start(project: &Path, fake_megara: &Path, agent_dir: &Path) -> Self {
        let extension =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("harness/pi/extensions/megara.ts");
        Self::start_with_extension(project, fake_megara, agent_dir, &extension)
    }

    fn start_with_extension(
        project: &Path,
        fake_megara: &Path,
        agent_dir: &Path,
        extension: &Path,
    ) -> Self {
        let mut child = Command::new(std::env::var("PI_BIN").unwrap_or_else(|_| "pi".to_string()))
            .args([
                "--mode",
                "rpc",
                "--no-session",
                "--no-context-files",
                "--no-skills",
                "--no-themes",
                "--no-prompt-templates",
                "--no-extensions",
                "--offline",
                "--extension",
            ])
            .arg(extension)
            .current_dir(project)
            .env("MEGARA_BIN", fake_megara)
            .env("PI_FAKE_LOG", project.join("fake.log"))
            .env("PI_CHILD_PID", project.join("child.pid"))
            .env("PI_CODING_AGENT_DIR", agent_dir)
            .env("PI_OFFLINE", "1")
            .env("PI_TELEMETRY", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn send(&mut self, value: Value) {
        writeln!(self.stdin, "{value}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn next(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "Pi RPC host closed stdout");
        serde_json::from_str(line.trim_end()).unwrap()
    }

    fn slash(
        &mut self,
        id: &str,
        command: &str,
        input: Option<&str>,
        confirmed: bool,
    ) -> Vec<Value> {
        self.send(json!({"id":id,"type":"prompt","message":command}));
        let mut responses = Vec::new();
        loop {
            let response = self.next();
            let done = response["type"] == "response" && response["id"] == id;
            if response["type"] == "extension_ui_request" {
                match response["method"].as_str() {
                    Some("input") => {
                        if let Some(input) = input {
                            self.send(json!({
                                "type":"extension_ui_response",
                                "id":response["id"],
                                "value":input
                            }));
                        } else {
                            self.send(json!({
                                "type":"extension_ui_response",
                                "id":response["id"],
                                "cancelled":true
                            }));
                        }
                    }
                    Some("confirm") => {
                        assert!(response["message"].as_str().unwrap().contains("argv:"));
                        self.send(json!({
                            "type":"extension_ui_response",
                            "id":response["id"],
                            "confirmed":confirmed
                        }));
                    }
                    _ => {}
                }
            }
            responses.push(response);
            if done {
                return responses;
            }
        }
    }

    fn finish(mut self) {
        drop(self.stdin);
        let status = self.child.wait().unwrap();
        let mut stderr = Vec::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(status.success(), "Pi status={status:?} stderr={stderr:?}");
        assert!(stderr.is_empty(), "Pi stderr={stderr:?}");
    }
}

fn fake_megara(project: &Path) -> PathBuf {
    let path = project.join("fake-megara");
    fs::write(
        &path,
        r##"#!/bin/sh
set -eu
printf 'CALL\n' >> "$PI_FAKE_LOG"
for arg do printf '%s\n' "$arg" >> "$PI_FAKE_LOG"; done
printf 'END\n' >> "$PI_FAKE_LOG"
if [ "${1-}" = "planning" ] && [ "${2-}" = "rpc" ]; then
  printf '%s\n' '{"protocol_version":1,"request_id":"fake","operation":"planning.status","ok":true,"session_id":"pln-test","revision":7,"replayed":false,"result":{"schema":"megara.result/v1","operation":"planning.status","state":{"spec":{"current_candidate":{"candidate_id":"spec-candidate","semantic_hash":"sha256:spec","base_domain_revision":1}}}},"observed":{"projection_status":"unchanged","evidence_current":true,"warnings":[]}}'
else
  printf '%s\n' '{"protocol_version":1,"request_id":"fake","ok":true,"replayed":false}'
fi
"##,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn calls(log: &Path) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    let mut current = None;
    for line in fs::read_to_string(log).unwrap().lines() {
        match line {
            "CALL" => current = Some(Vec::new()),
            "END" => result.push(current.take().unwrap()),
            argument => current.as_mut().unwrap().push(argument.to_string()),
        }
    }
    result
}

fn starts_with(call: &[String], prefix: &[&str]) -> bool {
    call.len() >= prefix.len()
        && call
            .iter()
            .zip(prefix)
            .all(|(actual, expected)| actual == expected)
}

#[path = "planning_pi_process.rs"]
mod process;

#[cfg(unix)]
#[test]
fn pi_rpc_user_commands_request_input_confirm_and_preserve_exact_argv() {
    let project = tempdir().unwrap();
    let agent_dir = tempdir().unwrap();
    let fake = fake_megara(project.path());
    let log = project.path().join("fake.log");
    let mut pi = PiRpc::start(project.path(), &fake, agent_dir.path());
    pi.send(json!({"id":"commands","type":"get_commands"}));
    let commands = pi.next();
    let names = commands["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(names.contains(&"megara-revise"));
    assert!(names.contains(&"megara-approve"));
    assert!(names.contains(&"megara-purge"));

    let cancelled = pi.slash("cancel", "/megara-revise spec pln-test", None, true);
    assert!(
        cancelled
            .iter()
            .any(|response| response["method"] == "input"),
        "responses={cancelled:?}"
    );
    assert!(!cancelled
        .iter()
        .any(|response| response["type"] == "extension_error"));
    assert!(!cancelled
        .iter()
        .any(|response| response["method"] == "confirm"));
    assert!(calls(&log).iter().all(|call| {
        !starts_with(call, &["planning", "spec", "revise"])
            && !starts_with(call, &["planning", "spec", "approve"])
            && !starts_with(call, &["planning", "purge"])
    }));

    let revised = pi.slash(
        "revise",
        "/megara-revise spec pln-test",
        Some("request a narrower scope"),
        true,
    );
    let confirm = revised
        .iter()
        .find(|response| response["method"] == "confirm")
        .unwrap();
    let message = confirm["message"].as_str().unwrap();
    assert!(message.contains("revise"), "confirm message={message}");
    assert!(message.contains("request a narrower scope"));

    let approved = pi.slash("approve", "/megara-approve spec pln-test", None, true);
    assert!(approved
        .iter()
        .any(|response| response["method"] == "confirm"));
    let purged = pi.slash("purge", "/megara-purge pln-test", None, true);
    assert!(purged
        .iter()
        .any(|response| response["method"] == "confirm"));
    pi.finish();

    let calls = calls(&log);
    let revise = calls
        .iter()
        .find(|call| starts_with(call, &["planning", "spec", "revise"]))
        .unwrap();
    assert!(revise
        .windows(2)
        .any(|pair| pair[0] == "--text" && pair[1] == "request a narrower scope"));
    let approve = calls
        .iter()
        .find(|call| starts_with(call, &["planning", "spec", "approve"]))
        .unwrap();
    assert!(approve
        .windows(2)
        .any(|pair| pair[0] == "--semantic-hash" && pair[1] == "sha256:spec"));
    let purge = calls
        .iter()
        .find(|call| starts_with(call, &["planning", "purge"]))
        .unwrap();
    assert!(purge
        .windows(2)
        .any(|pair| pair[0] == "--confirm" && pair[1] == "pln-test"));
}
