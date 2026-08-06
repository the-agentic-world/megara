use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::tempdir;

use super::PiRpc;

fn write_node_fixture(path: &Path, body: &str) {
    fs::write(path, format!("#!/usr/bin/env node\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn run_process_fixture(project: &Path, fake: &Path, label: &str, body: &str) -> Value {
    let pid_path = project.join(format!("child-{label}.pid"));
    let _ = fs::remove_file(&pid_path);
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("harness/pi/megara_process.ts");
    let extension = project.join("process-extension.ts");
    fs::copy(&helper, project.join("megara_process.ts")).unwrap();
    let source = format!(
        "import {{ existsSync }} from \"node:fs\";\nimport {{ runProcess, runProcessWithPolicy }} from \"./megara_process.js\";\n\nexport default function (pi) {{\n  pi.registerCommand(\"test-process\", {{\n    description: \"Pi process fixture\",\n    handler: async (_args, ctx) => {{\n      try {{\n        {body}\n      }} catch (error) {{\n        ctx.ui.notify(JSON.stringify({{ ok: false, error: String(error) }}), \"error\");\n      }}\n    }},\n  }});\n}}\n"
    );
    fs::write(&extension, source).unwrap();
    let agent_dir = tempdir().unwrap();
    let mut pi =
        PiRpc::start_with_extension_and_pid(project, fake, agent_dir.path(), &extension, &pid_path);
    let responses = pi.slash("process", "/test-process", None, true);
    let notification = responses
        .iter()
        .find(|response| response["method"] == "notify")
        .unwrap_or_else(|| panic!("process fixture responses={responses:?}"));
    let value = serde_json::from_str(notification["message"].as_str().unwrap()).unwrap();
    pi.finish();
    value
}

fn run_extension_command(
    project: &Path,
    fake: &Path,
    source: &str,
    command: &str,
    args: &[&str],
) -> Vec<Value> {
    let extension = project.join("command-fixture.ts");
    fs::write(&extension, source).unwrap();
    let agent_dir = tempdir().unwrap();
    let mut pi = PiRpc::start_with_extension(project, fake, agent_dir.path(), &extension);
    let message = if args.is_empty() {
        format!("/{command}")
    } else {
        format!("/{command} {}", args.join(" "))
    };
    let responses = pi.slash("fixture", &message, None, true);
    pi.finish();
    responses
}

fn notification_value(responses: &[Value]) -> Value {
    let notification = responses
        .iter()
        .find(|response| response["method"] == "notify")
        .unwrap_or_else(|| panic!("fixture responses={responses:?}"));
    serde_json::from_str(notification["message"].as_str().unwrap()).unwrap()
}

fn failing_megara(project: &Path) -> PathBuf {
    let path = project.join("failing-megara");
    fs::write(
        &path,
        "#!/bin/sh\nprintf 'simulated transport failure\\n' >&2\nexit 17\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn assert_reaped(project: &Path, label: &str) {
    let pid_path = project.join(format!("child-{label}.pid"));
    assert!(pid_path.is_file(), "child did not write its PID marker");
    let pid = fs::read_to_string(pid_path).unwrap();
    assert!(!Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
}

#[cfg(unix)]
#[test]
fn pi_process_transport_preserves_utf8_and_reaps_abort_and_overflow_children() {
    let project = tempdir().unwrap();
    let utf8_fake = project.path().join("utf8-child");
    write_node_fixture(
        &utf8_fake,
        r#"const bytes = Buffer.from('{"text":"안녕"}\n');
process.stdout.write(bytes.subarray(0, 10));
setTimeout(() => process.stdout.write(bytes.subarray(10)), 10);"#,
    );
    let utf8 = run_process_fixture(
        project.path(),
        &utf8_fake,
        "utf8",
        r#"const result = await runProcess(process.cwd(), ["planning", "rpc"], "x", undefined);
ctx.ui.notify(JSON.stringify({ok:true, result}), "info");"#,
    );
    assert_eq!(utf8["ok"], true);
    assert_eq!(utf8["result"]["stdout"], "{\"text\":\"안녕\"}\n");

    let overflow_fake = project.path().join("overflow-child");
    write_node_fixture(
        &overflow_fake,
        r#"require("fs").writeFileSync(process.env.PI_CHILD_PID, String(process.pid));
process.stdout.write(Buffer.alloc(4 * 1024 * 1024 + 1, 65));"#,
    );
    let overflow = run_process_fixture(
        project.path(),
        &overflow_fake,
        "stdout-overflow",
        r#"await runProcess(process.cwd(), ["planning", "rpc"], "x", undefined);
ctx.ui.notify(JSON.stringify({ok:true}), "info");"#,
    );
    assert_eq!(overflow["ok"], false);
    assert!(overflow["error"]
        .as_str()
        .unwrap()
        .contains("exceeded 4 MiB"));
    assert_reaped(project.path(), "stdout-overflow");

    let stderr_fake = project.path().join("stderr-child");
    write_node_fixture(
        &stderr_fake,
        r#"require("fs").writeFileSync(process.env.PI_CHILD_PID, String(process.pid));
process.stderr.write(Buffer.alloc(4 * 1024 * 1024 + 1, 66));"#,
    );
    let stderr = run_process_fixture(
        project.path(),
        &stderr_fake,
        "stderr-overflow",
        r#"await runProcess(process.cwd(), ["planning", "rpc"], "x", undefined);
ctx.ui.notify(JSON.stringify({ok:true}), "info");"#,
    );
    assert_eq!(stderr["ok"], false);
    assert!(stderr["error"]
        .as_str()
        .unwrap()
        .contains("stderr exceeded 4 MiB"));
    assert_reaped(project.path(), "stderr-overflow");

    let timeout_fake = project.path().join("timeout-child");
    write_node_fixture(
        &timeout_fake,
        r#"require("fs").writeFileSync(process.env.PI_CHILD_PID, String(process.pid));
process.on("SIGTERM", () => {});
setInterval(() => {}, 1000);"#,
    );
    let timeout = run_process_fixture(
        project.path(),
        &timeout_fake,
        "timeout",
        r#"await runProcessWithPolicy(process.cwd(), ["planning", "rpc"], "x", undefined, {timeoutMs: 5_000, terminationGraceMs: 50});
ctx.ui.notify(JSON.stringify({ok:true}), "info");"#,
    );
    assert_eq!(timeout["ok"], false);
    assert!(timeout["error"].as_str().unwrap().contains("timed out"));
    assert_reaped(project.path(), "timeout");

    let abort_fake = project.path().join("abort-child");
    write_node_fixture(
        &abort_fake,
        r#"require("fs").writeFileSync(process.env.PI_CHILD_PID, String(process.pid));
process.on("SIGTERM", () => {});
setInterval(() => {}, 1000);"#,
    );
    let aborted = run_process_fixture(
        project.path(),
        &abort_fake,
        "abort",
        r#"const controller = new AbortController();
const pending = runProcessWithPolicy(process.cwd(), ["planning", "rpc"], "x", controller.signal, {timeoutMs: 120_000, terminationGraceMs: 50});
const pidFile = process.env.PI_CHILD_PID!;
const deadline = Date.now() + 5_000;
while (!existsSync(pidFile) && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 10));
if (!existsSync(pidFile)) {
  controller.abort();
  await pending.catch(() => undefined);
  throw new Error("child did not become ready");
}
controller.abort();
await pending;
ctx.ui.notify(JSON.stringify({ok:true}), "info");"#,
    );
    assert_eq!(aborted["ok"], false);
    assert!(aborted["error"].as_str().unwrap().contains("was aborted"));
    assert_reaped(project.path(), "abort");

    let epipe_fake = project.path().join("epipe-child");
    write_node_fixture(
        &epipe_fake,
        r#"require("fs").writeFileSync(process.env.PI_CHILD_PID, String(process.pid));
process.stdin.destroy();
setTimeout(() => process.exit(0), 1000);"#,
    );
    let epipe = run_process_fixture(
        project.path(),
        &epipe_fake,
        "epipe",
        r#"await runProcess(process.cwd(), ["planning", "rpc"], "x".repeat(8 * 1024 * 1024), undefined);
ctx.ui.notify(JSON.stringify({ok:true}), "info");"#,
    );
    assert_eq!(epipe["ok"], false);
    assert!(epipe["error"].as_str().unwrap().contains("IO_ERROR"));
    assert_reaped(project.path(), "epipe");
}

#[cfg(unix)]
#[test]
fn pi_typed_transport_failure_and_stable_command_id_run_in_real_host() {
    let project = tempdir().unwrap();
    let fake = failing_megara(project.path());
    let main = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("harness/pi/extensions/megara.ts");
    let main_literal = format!("{:?}", main.display().to_string());
    let typed_source = format!(
        r#"import megara from {main_literal};
export default function (pi) {{
  pi.registerCommand("test-tool-error", {{
    description: "invoke the real planning tool boundary",
    handler: async (_args, ctx) => {{
      const tools = [];
      megara({{
        registerTool(tool) {{ tools.push(tool); }},
        registerCommand() {{}},
      }});
      const tool = tools.find((entry) => entry.name === "planning_start");
      const result = await tool.execute(
        "opaque-tool-call",
        {{ request: "transport failure" }},
        undefined,
        () => {{}},
        {{ cwd: process.cwd() }},
      );
      ctx.ui.notify(JSON.stringify(result), "error");
    }},
  }});
}}
"#
    );
    let responses =
        run_extension_command(project.path(), &fake, &typed_source, "test-tool-error", &[]);
    assert!(!responses
        .iter()
        .any(|response| response["type"] == "extension_error"));
    let result = notification_value(&responses);
    let details = &result["details"];
    assert_eq!(details["ok"], false);
    assert_eq!(details["error"]["code"], "IO_ERROR");
    assert_eq!(details["error"]["retryable"], false);
    assert_eq!(details["error"]["details"]["transport"], "pi");
    assert!(details["state"].is_null());
    let content =
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(content, details.clone());
    assert!(!project.path().join(".megara").exists());

    let stable_source = format!(
        r#"import {{ stableCommandId }} from {main_literal};
export default function (pi) {{
  pi.registerCommand("test-command-id", {{
    description: "derive stable command IDs",
    handler: async (args, ctx) => {{
      ctx.ui.notify(JSON.stringify(args.trim().split(/\s+/).filter(Boolean).map(stableCommandId)), "info");
    }},
  }});
}}
"#
    );
    let long_id = "x".repeat(129);
    let responses = run_extension_command(
        project.path(),
        &fake,
        &stable_source,
        "test-command-id",
        &["a!b", "a?b", &long_id, "a!b"],
    );
    let ids = notification_value(&responses);
    let ids = ids.as_array().unwrap();
    assert_eq!(ids.len(), 4, "ids={ids:?} responses={responses:?}");
    assert_ne!(ids[0], ids[1]);
    assert_eq!(ids[0], ids[3]);
    for id in ids {
        let id = id.as_str().unwrap();
        assert!(id.starts_with("cmd_pi_"));
        assert_eq!(id.len(), 71);
        assert!(id.len() <= 128);
    }
}
