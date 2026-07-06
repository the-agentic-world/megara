use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_yaml::Value;

use crate::{
    cli::{DocsCheckArgs, DocsInitArgs},
    ui::{self, Section},
};

const INDEX_FILE: &str = "index.md";
const LOG_FILE: &str = "log.md";

#[derive(Debug, Serialize)]
pub struct DocsInitReport {
    pub root: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub conflicts: Vec<String>,
    #[serde(skip)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
pub struct DocsCheckReport {
    pub root: String,
    pub ok: bool,
    pub checked: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub json: bool,
}

pub fn init(args: DocsInitArgs) -> Result<()> {
    let root = resolve_root(args.root)?;
    let report = init_bundle(&root, args.force, args.json)?;
    let has_conflicts = !report.conflicts.is_empty();
    report.print()?;
    if has_conflicts {
        bail!(
            "refusing to overwrite {} existing docs file(s); rerun with --force",
            report.conflicts.len()
        );
    }
    Ok(())
}

pub fn check(args: DocsCheckArgs) -> Result<()> {
    let root = resolve_root(args.root)?;
    let report = check_bundle(&root, args.json)?;
    let ok = report.ok;
    report.print()?;
    if !ok {
        bail!("OKF docs check failed");
    }
    Ok(())
}

pub(crate) fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf> {
    let root = root.unwrap_or_else(|| PathBuf::from("docs"));
    if root.is_absolute() {
        Ok(root)
    } else {
        Ok(std::env::current_dir()?.join(root))
    }
}

pub(crate) fn init_bundle(root: &Path, force: bool, json: bool) -> Result<DocsInitReport> {
    let timestamp = timestamp();
    let files = [
        (root.join(INDEX_FILE), index_scaffold(&timestamp)),
        (root.join(LOG_FILE), log_scaffold(&timestamp)),
    ];
    let mut report = DocsInitReport {
        root: root.display().to_string(),
        created: Vec::new(),
        updated: Vec::new(),
        unchanged: Vec::new(),
        conflicts: Vec::new(),
        json,
    };

    for (path, content) in files {
        write_scaffold(&path, &content, force, &mut report)?;
    }

    Ok(report)
}

pub(crate) fn check_bundle(root: &Path, json: bool) -> Result<DocsCheckReport> {
    let mut report = DocsCheckReport {
        root: root.display().to_string(),
        ok: true,
        checked: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
        json,
    };

    if !root.exists() {
        report
            .errors
            .push(format!("docs root does not exist: {}", root.display()));
        report.ok = false;
        return Ok(report);
    }

    check_reserved_file(root, INDEX_FILE, &mut report);
    check_reserved_file(root, LOG_FILE, &mut report);
    for path in markdown_files(root)? {
        if is_reserved_file(&path) || is_excluded_path(root, &path) {
            continue;
        }
        check_concept_file(&path, &mut report);
    }

    report.ok = report.errors.is_empty();
    Ok(report)
}

fn write_scaffold(
    path: &Path,
    content: &str,
    force: bool,
    report: &mut DocsInitReport,
) -> Result<()> {
    if path.exists() {
        let current = fs::read_to_string(path)
            .with_context(|| format!("failed to read existing docs file {}", path.display()))?;
        if current == content {
            report.unchanged.push(path.display().to_string());
            return Ok(());
        }
        if !force {
            report.conflicts.push(path.display().to_string());
            return Ok(());
        }
        write_text(path, content)?;
        report.updated.push(path.display().to_string());
        return Ok(());
    }

    write_text(path, content)?;
    report.created.push(path.display().to_string());
    Ok(())
}

fn write_text(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let mut file = fs::File::create(path)
        .with_context(|| format!("failed to create docs file {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("failed to write docs file {}", path.display()))?;
    Ok(())
}

fn check_reserved_file(root: &Path, name: &str, report: &mut DocsCheckReport) {
    let path = root.join(name);
    if !path.exists() {
        report
            .errors
            .push(format!("missing reserved OKF file: {}", path.display()));
        return;
    }
    match fs::read_to_string(&path) {
        Ok(content) if content.trim().is_empty() => {
            report
                .errors
                .push(format!("reserved OKF file is empty: {}", path.display()));
        }
        Ok(content) if !content.lines().any(|line| line.starts_with("# ")) => {
            report.warnings.push(format!(
                "reserved OKF file has no H1 heading: {}",
                path.display()
            ));
        }
        Ok(_) => report.checked.push(path.display().to_string()),
        Err(error) => report
            .errors
            .push(format!("failed to read {}: {error}", path.display())),
    }
}

fn check_concept_file(path: &Path, report: &mut DocsCheckReport) {
    report.checked.push(path.display().to_string());
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            report
                .errors
                .push(format!("failed to read {}: {error}", path.display()));
            return;
        }
    };
    let Some(frontmatter) = frontmatter(&content) else {
        report.errors.push(format!(
            "missing YAML frontmatter in OKF concept: {}",
            path.display()
        ));
        return;
    };
    let value: Value = match serde_yaml::from_str(frontmatter) {
        Ok(value) => value,
        Err(error) => {
            report.errors.push(format!(
                "invalid YAML frontmatter in {}: {error}",
                path.display()
            ));
            return;
        }
    };

    if string_field(&value, "type").is_none_or(str::is_empty) {
        report
            .errors
            .push(format!("missing required OKF type: {}", path.display()));
    }
    for field in ["title", "description", "tags", "timestamp"] {
        if !has_field(&value, field) {
            report.warnings.push(format!(
                "missing recommended OKF field `{field}`: {}",
                path.display()
            ));
        }
    }
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_markdown(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_markdown(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_markdown(&entry?.path(), files)?;
        }
        return Ok(());
    }
    if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn is_reserved_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, INDEX_FILE | LOG_FILE))
}

fn is_excluded_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if relative
        .components()
        .next()
        .and_then(component_name)
        .is_some_and(|component| component == "harness")
        && is_megara_product_harness(root)
    {
        return true;
    }
    let mut components = relative.components().filter_map(component_name);
    while let Some(component) = components.next() {
        if component == ".megara" {
            return true;
        }
        if component == ".agents" {
            return components
                .next()
                .is_some_and(|next| matches!(next, "state" | "skills"));
        }
    }
    false
}

fn is_megara_product_harness(root: &Path) -> bool {
    let harness = root.join("harness");
    harness.join("megara.toml").exists()
        && harness.join("skills").is_dir()
        && harness.join("agents").is_dir()
}

fn component_name(component: Component<'_>) -> Option<&str> {
    match component {
        Component::Normal(name) => name.to_str(),
        _ => None,
    }
}

fn frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some(&rest[..end])
}

fn has_field(value: &Value, key: &str) -> bool {
    field(value, key).is_some()
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    field(value, key).and_then(Value::as_str).map(str::trim)
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let mapping = value.as_mapping()?;
    mapping.get(Value::String(key.to_string()))
}

fn index_scaffold(timestamp: &str) -> String {
    format!(
        "---\ntype: KnowledgeIndex\ntitle: Project Knowledge Index\ndescription: Entry point for this OKF knowledge bundle.\ntimestamp: {timestamp}\ntags: [okf, index]\n---\n\n# Project Knowledge Index\n\n- [Knowledge log](log.md)\n"
    )
}

fn log_scaffold(timestamp: &str) -> String {
    format!(
        "---\ntype: KnowledgeLog\ntitle: Project Knowledge Log\ndescription: Chronological notes for this OKF knowledge bundle.\ntimestamp: {timestamp}\ntags: [okf, log]\n---\n\n# Project Knowledge Log\n\n- {timestamp}: OKF bundle initialized.\n"
    )
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

impl DocsInitReport {
    fn print(&self) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }
        let rows = [("root", self.root.clone())];
        let status = if self.conflicts.is_empty() {
            "OK"
        } else {
            "conflicts"
        };
        let sections = [
            Section::new("Created", self.created.clone()),
            Section::new("Updated", self.updated.clone()),
            Section::new("Unchanged", self.unchanged.clone()),
            Section::new("Conflicts", self.conflicts.clone()),
        ];
        ui::print_dashboard("Docs Init", status, &rows, &sections)
    }
}

impl DocsCheckReport {
    fn print(&self) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }
        let rows = [
            ("root", self.root.clone()),
            ("checked", self.checked.len().to_string()),
        ];
        let status = if self.ok { "OK" } else { "issues found" };
        let sections = [
            Section::new("Errors", self.errors.clone()),
            Section::new("Warnings", self.warnings.clone()),
        ];
        ui::print_dashboard("Docs Check", status, &rows, &sections)
    }
}
