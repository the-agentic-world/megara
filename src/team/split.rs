use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    cli::{TeamSplitArgs, TeamTeammateArgs},
    ui::{self, Section},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SplitTransport {
    Cmux,
    Tmux,
    Orca,
}

impl SplitTransport {
    fn as_str(self) -> &'static str {
        match self {
            SplitTransport::Cmux => "cmux",
            SplitTransport::Tmux => "tmux",
            SplitTransport::Orca => "orca",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SplitPrepareReport {
    pub status: &'static str,
    pub transport: String,
    pub roles: Vec<String>,
    pub commands: Vec<String>,
    pub opened: bool,
    #[serde(skip)]
    pub json: bool,
}

pub(crate) fn prepare_from_cli(args: TeamSplitArgs) -> Result<SplitPrepareReport> {
    let cwd = args
        .cwd
        .unwrap_or(env::current_dir().context("failed to read current directory")?);
    let runtime_root = args.runtime_root.unwrap_or_else(|| cwd.join(".megara"));
    let megara_bin = args
        .megara_bin
        .or_else(|| env::var("MEGARA_BIN").ok())
        .unwrap_or_else(|| ".agents/bin/megara".to_string());
    let task = match args.task {
        Some(task) => task,
        None => fs::read_to_string(
            team_runtime_dir(&runtime_root, &args.correlation_id).join("task.md"),
        )
        .with_context(|| {
            "team task is missing; pass --task or start team through Megara hooks first"
        })?,
    };
    let roles = normalize_roles(args.roles)?;
    let transport = resolve_transport(Some(args.transport.as_str()))?;
    let request = SplitPrepareRequest {
        cwd,
        runtime_root,
        roles,
        task,
        correlation_id: args.correlation_id,
        codex_bin: args.codex_bin,
        megara_bin,
        transport,
        open: args.open,
        json: args.json,
    };
    prepare(request)
}

pub(crate) fn run_teammate_from_cli(args: TeamTeammateArgs) -> Result<()> {
    let cwd = args
        .cwd
        .unwrap_or(env::current_dir().context("failed to read current directory")?);
    let assignment = fs::read_to_string(&args.assignment_file).with_context(|| {
        format!(
            "failed to read assignment {}",
            args.assignment_file.display()
        )
    })?;
    fs::create_dir_all(&args.receipt_dir)?;
    let output = Command::new(&args.codex_bin)
        .args(codex_exec_args(&args.role, &assignment)?)
        .current_dir(&cwd)
        .output();

    let timestamp = timestamp();
    let content_file = args
        .receipt_dir
        .join(format!("{}-{}.md", args.role, args.teammate_id));
    let receipt_file = args
        .receipt_dir
        .join(format!("{}-{}.json", args.role, args.teammate_id));
    let (kind, status, content) = match output {
        Ok(output) if output.status.success() => (
            "teammate-result",
            "succeeded",
            String::from_utf8_lossy(&output.stdout).to_string(),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            (
                "teammate-failure",
                "failed",
                format!("stdout:\n{stdout}\n\nstderr:\n{stderr}"),
            )
        }
        Err(error) => (
            "teammate-failure",
            "failed",
            format!("failed to run {}: {error}", args.codex_bin),
        ),
    };
    fs::write(&content_file, content)?;
    fs::write(
        &receipt_file,
        serde_json::to_string_pretty(&json!({
            "kind": kind,
            "status": status,
            "transport": args.transport.unwrap_or_else(|| "split-pane".to_string()),
            "workflow": "team",
            "role": args.role,
            "teammate_id": args.teammate_id,
            "correlation_id": args.correlation_id,
            "orchestration_request_id": args.correlation_id,
            "stopped_at": timestamp,
            "content_file": content_file,
        }))?,
    )?;
    println!("team teammate receipt recorded: {status}");
    Ok(())
}

pub(crate) fn codex_exec_args(role: &str, assignment: &str) -> Result<Vec<String>> {
    let profile = crate::targets::codex::role_profile(role)
        .ok_or_else(|| anyhow!("unknown Codex role profile: {role}"))?;
    Ok(vec![
        "exec".to_string(),
        "--model".to_string(),
        profile.model.to_string(),
        "--config".to_string(),
        format!("model_reasoning_effort=\"{}\"", profile.reasoning_effort),
        assignment.to_string(),
    ])
}

impl SplitPrepareReport {
    pub(crate) fn print(&self) -> Result<()> {
        ui::print_dashboard(
            "Team",
            self.status,
            &[
                ("transport", self.transport.clone()),
                ("opened", self.opened.to_string()),
            ],
            &[Section::new(
                "Split",
                vec![
                    format!("roles: {}", self.roles.join(", ")),
                    format!("commands: {}", self.commands.len()),
                ],
            )],
        )
    }
}

#[derive(Debug)]
struct SplitPrepareRequest {
    cwd: PathBuf,
    runtime_root: PathBuf,
    roles: Vec<String>,
    task: String,
    correlation_id: String,
    codex_bin: String,
    megara_bin: String,
    transport: SplitTransport,
    open: bool,
    json: bool,
}

fn prepare(request: SplitPrepareRequest) -> Result<SplitPrepareReport> {
    if request.roles.is_empty() {
        bail!("at least one teammate role is required");
    }
    let root = team_runtime_dir(&request.runtime_root, &request.correlation_id);
    let assignment_dir = root.join("assignments");
    let receipt_dir = root.join("receipts");
    fs::create_dir_all(&assignment_dir)?;
    fs::create_dir_all(&receipt_dir)?;
    fs::write(root.join("task.md"), &request.task)?;

    let pane_specs = request
        .roles
        .iter()
        .enumerate()
        .map(|(index, role)| {
            let teammate_id = format!("{role}-{}", index + 1);
            let assignment_file = assignment_dir.join(format!("{teammate_id}.md"));
            fs::write(
                &assignment_file,
                teammate_assignment(role, &teammate_id, &request.correlation_id, &request.task),
            )?;
            Ok(PaneSpec {
                role: role.clone(),
                teammate_id,
                assignment_file,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let teammate_commands = pane_specs
        .iter()
        .map(|pane| teammate_command(&request, pane, &receipt_dir))
        .collect::<Vec<_>>();
    let commands = split_commands(request.transport, &teammate_commands);

    let mut opened = false;
    if request.open {
        run_split_commands(request.transport, &teammate_commands)?;
        opened = true;
    }

    Ok(SplitPrepareReport {
        status: "prepared",
        transport: request.transport.as_str().to_string(),
        roles: request.roles,
        commands,
        opened,
        json: request.json,
    })
}

struct PaneSpec {
    role: String,
    teammate_id: String,
    assignment_file: PathBuf,
}

fn teammate_command(request: &SplitPrepareRequest, pane: &PaneSpec, receipt_dir: &Path) -> String {
    shell_join(&[
        &request.megara_bin,
        "team",
        "teammate",
        "--transport",
        request.transport.as_str(),
        "--role",
        &pane.role,
        "--teammate-id",
        &pane.teammate_id,
        "--correlation-id",
        &request.correlation_id,
        "--assignment-file",
        &pane.assignment_file.display().to_string(),
        "--receipt-dir",
        &receipt_dir.display().to_string(),
        "--cwd",
        &request.cwd.display().to_string(),
        "--codex-bin",
        &request.codex_bin,
    ])
}

fn split_commands(transport: SplitTransport, teammate_commands: &[String]) -> Vec<String> {
    teammate_commands
        .iter()
        .enumerate()
        .map(|(index, command)| match transport {
            SplitTransport::Cmux if index == 0 => {
                format!(
                    "cmux new-split right --focus true && env -u CMUX_SURFACE_ID cmux send {}",
                    shell_quote(&cmux_send_text(command))
                )
            }
            SplitTransport::Cmux => {
                format!(
                    "env -u CMUX_SURFACE_ID cmux new-split down --focus true && env -u CMUX_SURFACE_ID cmux send {}",
                    shell_quote(&cmux_send_text(command))
                )
            }
            SplitTransport::Tmux if index == 0 => {
                format!("tmux split-window -h {}", shell_quote(command))
            }
            SplitTransport::Tmux => {
                format!("tmux split-window -v {}", shell_quote(command))
            }
            SplitTransport::Orca if index == 0 => format!(
                "orca terminal split --direction horizontal --command {} --json",
                shell_quote(command)
            ),
            SplitTransport::Orca => format!(
                "orca terminal split --direction vertical --command {} --json",
                shell_quote(command)
            ),
        })
        .collect()
}

fn run_split_commands(transport: SplitTransport, teammate_commands: &[String]) -> Result<()> {
    match transport {
        SplitTransport::Cmux => run_cmux(teammate_commands),
        SplitTransport::Tmux => run_tmux(teammate_commands),
        SplitTransport::Orca => run_orca(teammate_commands),
    }
}

fn run_cmux(teammate_commands: &[String]) -> Result<()> {
    for (index, command) in teammate_commands.iter().enumerate() {
        let direction = if index == 0 { "right" } else { "down" };
        let mut split = Command::new("cmux");
        split.args(["new-split", direction, "--focus", "true"]);
        if index > 0 {
            split.env_remove("CMUX_SURFACE_ID");
        }
        checked(&mut split, "cmux new-split")?;
        let mut send = Command::new("cmux");
        send.env_remove("CMUX_SURFACE_ID");
        send.args(["send", &cmux_send_text(command)]);
        checked(&mut send, "cmux send")?;
    }
    Ok(())
}

fn cmux_send_text(command: &str) -> String {
    format!("{command}\\n")
}

fn run_tmux(teammate_commands: &[String]) -> Result<()> {
    if env::var_os("TMUX").is_none() {
        bail!("tmux transport requires TMUX environment");
    }
    for (index, command) in teammate_commands.iter().enumerate() {
        let direction = if index == 0 { "-h" } else { "-v" };
        checked(
            Command::new("tmux").args(["split-window", direction, command]),
            "tmux split-window",
        )?;
    }
    Ok(())
}

fn run_orca(teammate_commands: &[String]) -> Result<()> {
    let mut target_terminal: Option<String> = None;
    for (index, command) in teammate_commands.iter().enumerate() {
        let direction = if index == 0 { "horizontal" } else { "vertical" };
        let mut process = Command::new("orca");
        process.args([
            "terminal",
            "split",
            "--direction",
            direction,
            "--command",
            command,
            "--json",
        ]);
        if let Some(terminal) = target_terminal.as_ref() {
            process.args(["--terminal", terminal]);
        }
        let output = process
            .output()
            .context("failed to run orca terminal split")?;
        if !output.status.success() {
            bail!(
                "orca terminal split failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if target_terminal.is_none() {
            target_terminal = terminal_handle_from_json(&output.stdout);
        }
    }
    Ok(())
}

fn checked(command: &mut Command, label: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("failed to run {label}"))?;
    if !output.status.success() {
        bail!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn terminal_handle_from_json(bytes: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<Value>(bytes).ok()?;
    find_string_key(&value, &["handle", "terminal", "terminal_id", "id"])
}

fn find_string_key(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in keys {
                if let Some(value) = object.get(*key).and_then(Value::as_str) {
                    return Some(value.to_string());
                }
            }
            object
                .values()
                .find_map(|value| find_string_key(value, keys))
        }
        Value::Array(items) => items.iter().find_map(|value| find_string_key(value, keys)),
        _ => None,
    }
}

fn teammate_assignment(role: &str, teammate_id: &str, correlation_id: &str, task: &str) -> String {
    format!(
        "You are a Megara team teammate.\n\
Role: {role}\n\
Teammate id: {teammate_id}\n\
Correlation id: {correlation_id}\n\n\
Task:\n{task}\n\n\
Return a concise teammate result or teammate failure. Include what you checked, what changed if anything, verification evidence, and blockers. Keep Megara runtime metadata out of user-facing prose. Do not invoke Megara workflows. Non-executor roles should avoid file writes unless the assignment explicitly requires them.\n"
    )
}

fn normalize_roles(values: Vec<String>) -> Result<Vec<String>> {
    let roles = values
        .into_iter()
        .map(|role| {
            crate::team::parse_role(&role)
                .map(|role| role.as_str().to_string())
                .ok_or_else(|| anyhow!("unknown team role: {role}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut deduped = Vec::new();
    for role in roles {
        if !deduped.contains(&role) {
            deduped.push(role);
        }
    }
    Ok(deduped)
}

fn resolve_transport(value: Option<&str>) -> Result<SplitTransport> {
    match value.unwrap_or("auto") {
        "auto" => auto_transport(),
        "cmux" => Ok(SplitTransport::Cmux),
        "tmux" => Ok(SplitTransport::Tmux),
        "orca" => Ok(SplitTransport::Orca),
        other => bail!("unsupported split transport: {other}; expected auto, cmux, tmux, or orca"),
    }
}

fn auto_transport() -> Result<SplitTransport> {
    if env::var_os("CMUX_WORKSPACE_ID").is_some() || command_ok("cmux", &["ping"]) {
        return Ok(SplitTransport::Cmux);
    }
    if env::var_os("TMUX").is_some() {
        return Ok(SplitTransport::Tmux);
    }
    if command_ok("orca", &["status", "--json"]) {
        return Ok(SplitTransport::Orca);
    }
    bail!("no supported CLI split pane transport found; expected cmux, tmux, or orca")
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn team_runtime_dir(runtime_root: &Path, correlation_id: &str) -> PathBuf {
    runtime_root
        .join("state")
        .join("team")
        .join("split")
        .join(safe_name(correlation_id))
}

pub(crate) fn receipt_dir(runtime_root: &Path, correlation_id: &str) -> PathBuf {
    team_runtime_dir(runtime_root, correlation_id).join("receipts")
}

pub(crate) fn task_file(runtime_root: &Path, correlation_id: &str) -> PathBuf {
    team_runtime_dir(runtime_root, correlation_id).join("task.md")
}

pub(crate) fn write_task(runtime_root: &Path, correlation_id: &str, task: &str) -> Result<()> {
    let path = task_file(runtime_root, correlation_id);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| anyhow!("team task path has no parent"))?,
    )?;
    fs::write(path, task)?;
    Ok(())
}

fn shell_join(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn safe_name(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('-').to_string();
    if safe.is_empty() {
        "team".to_string()
    } else {
        safe
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
