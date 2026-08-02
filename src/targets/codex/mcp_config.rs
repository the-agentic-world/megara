use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::installer::ManagedTomlEdit;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use toml_edit::{value, Array, DocumentMut, Item, Table, Value as EditValue};

const MCP_HASH_PREFIX: &str = "# MEGARA:MCP-SHA256=";

pub(super) fn plan(root: &Path, executable: &Path, force: bool) -> Result<ManagedTomlEdit> {
    let path = root.join("config.toml");
    let (source, existing, permissions) = match read_existing(&path)? {
        Some((source, existing, permissions)) => (Some(source), Some(existing), Some(permissions)),
        None => (None, None, None),
    };
    let project_root = root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf())
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());
    render(
        &path,
        source.as_deref(),
        existing.as_deref(),
        permissions,
        executable,
        &project_root,
        force,
    )
}

pub(super) fn plan_remove(root: &Path, force: bool) -> Result<Option<ManagedTomlEdit>> {
    let path = root.join("config.toml");
    let Some((source, existing, permissions)) = read_existing(&path)? else {
        return Ok(None);
    };
    let (without_hash, stored_hash) = remove_hash_line(&existing);
    let mut document: DocumentMut = without_hash
        .parse()
        .context("failed to parse Codex config TOML")?;
    let Some(servers) = document.get_mut("mcp_servers").and_then(Item::as_table_mut) else {
        return Ok(None);
    };
    let Some(current) = servers.get("megara_planning") else {
        return Ok(None);
    };
    let current_hash = item_hash(current)?;
    let managed = stored_hash.as_deref() == Some(current_hash.as_str());
    if !managed && !force {
        bail!("refusing to remove edited or unmanaged megara_planning MCP table");
    }
    let backup = (force && !managed)
        .then(|| table_backup(&existing))
        .transpose()?;
    servers.remove("megara_planning");
    let desired = document.to_string();
    Ok(Some(ManagedTomlEdit {
        path: path.clone(),
        created: false,
        changed: desired.as_bytes() != source.as_slice(),
        backup_path: backup.as_ref().map(|_| backup_path(&path)),
        desired,
        backup,
        expected_source: Some(source),
        permissions: Some(permissions),
    }))
}

fn render(
    path: &Path,
    source: Option<&[u8]>,
    existing: Option<&str>,
    permissions: Option<fs::Permissions>,
    executable: &Path,
    project_root: &Path,
    force: bool,
) -> Result<ManagedTomlEdit> {
    let base = existing.unwrap_or("# Megara Codex projection.\n");
    let (without_hash, stored_hash) = remove_hash_line(base);
    let mut document: DocumentMut = without_hash
        .parse()
        .context("failed to parse Codex config TOML")?;
    let current = document
        .get("mcp_servers")
        .and_then(Item::as_table)
        .and_then(|servers| servers.get("megara_planning"));
    let current_hash = current.map(item_hash).transpose()?;
    let unmanaged = current_hash.as_deref() != stored_hash.as_deref();
    if current.is_some() && unmanaged && !force {
        bail!("unmanaged or directly edited megara_planning MCP table; rerun with --force");
    }
    let backup = if current.is_some() && unmanaged && force {
        Some(table_backup(base)?)
    } else {
        None
    };
    let servers = document["mcp_servers"].or_insert(Item::Table(Table::new()));
    let servers = servers
        .as_table_mut()
        .context("mcp_servers must be a TOML table")?;
    servers.insert("megara_planning", desired_item(executable, project_root));
    let desired_hash = item_hash(
        servers
            .get("megara_planning")
            .expect("MCP table was inserted"),
    )?;
    let mut desired = document.to_string();
    if !desired.ends_with('\n') {
        desired.push('\n');
    }
    desired.push_str(MCP_HASH_PREFIX);
    desired.push_str(&desired_hash);
    desired.push('\n');
    let changed = desired.as_bytes() != base.as_bytes();
    Ok(ManagedTomlEdit {
        path: path.to_path_buf(),
        created: existing.is_none(),
        changed,
        backup_path: backup.as_ref().map(|_| backup_path(path)),
        desired,
        backup,
        expected_source: source.map(ToOwned::to_owned),
        permissions,
    })
}

fn desired_item(executable: &Path, project_root: &Path) -> Item {
    let mut table = Table::new();
    table.insert("command", value(executable.display().to_string()));
    let mut args = Array::default();
    args.push("planning");
    args.push("mcp");
    args.push("--project");
    args.push(project_root.display().to_string());
    table.insert("args", Item::Value(EditValue::Array(args)));
    table.insert("cwd", value(project_root.display().to_string()));
    table.insert("enabled", value(true));
    table.insert("startup_timeout_sec", value(10));
    table.insert("tool_timeout_sec", value(120));
    let mut tools = Table::new();
    for name in [
        "planning_spec_approve",
        "planning_plan_approve",
        "planning_purge",
    ] {
        let mut config = Table::new();
        config.insert("approval_mode", value("prompt"));
        tools.insert(name, Item::Table(config));
    }
    table.insert("tools", Item::Table(tools));
    Item::Table(table)
}

fn item_hash(item: &Item) -> Result<String> {
    let mut document = DocumentMut::new();
    let mut servers = Table::new();
    servers.insert("megara_planning", item.clone());
    document["mcp_servers"] = Item::Table(servers);
    let mut hasher = Sha256::new();
    hasher.update(document.to_string().as_bytes());
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn table_backup(content: &str) -> Result<Vec<u8>> {
    let header = "[mcp_servers.megara_planning]";
    let mut start = None;
    let mut end = None;
    let mut offset = 0;
    for segment in content.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed = line.trim();
        if start.is_none() {
            if trimmed == header {
                start = Some(offset);
            }
        } else if (trimmed.starts_with('[')
            && !trimmed.starts_with("[mcp_servers.megara_planning."))
            || trimmed.starts_with(MCP_HASH_PREFIX)
        {
            end = Some(offset);
            break;
        }
        offset += segment.len();
    }
    let Some(start) = start else {
        bail!("cannot isolate managed megara_planning TOML table for backup");
    };
    let end = end.unwrap_or(content.len());
    Ok(content.as_bytes()[start..end].to_vec())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name("config.toml.megara.mcp.bak")
}

fn remove_hash_line(content: &str) -> (String, Option<String>) {
    let mut result = String::new();
    let mut hash = None;
    for segment in content.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(value) = line.strip_prefix(MCP_HASH_PREFIX) {
            hash = Some(value.trim().to_string());
        } else {
            result.push_str(segment);
        }
    }
    (result, hash)
}

fn read_existing(path: &Path) -> Result<Option<(Vec<u8>, String, fs::Permissions)>> {
    let source = match fs::read(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read Codex config {}", path.display()))
        }
    };
    let content = String::from_utf8(source.clone())
        .with_context(|| format!("Codex config is not valid UTF-8: {}", path.display()))?;
    let permissions = fs::metadata(path)
        .with_context(|| format!("failed to stat Codex config {}", path.display()))?
        .permissions();
    Ok(Some((source, content, permissions)))
}
