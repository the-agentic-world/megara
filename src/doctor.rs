use std::fs;

use anyhow::Result;
use serde::Serialize;

use crate::{
    installer::{DoctorOptions, InstallOptions, Planner, MANAGED_MARKER},
    templates::TemplateRegistry,
};

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub scope: String,
    pub target: String,
    pub ok: bool,
    pub missing: Vec<String>,
    pub unmanaged: Vec<String>,
    pub stale: Vec<String>,
    #[serde(skip)]
    pub json: bool,
}

pub fn run(registry: &TemplateRegistry, options: DoctorOptions) -> Result<DoctorReport> {
    let install_options = InstallOptions {
        scope: options.scope,
        target: options.target,
        dry_run: true,
        force: false,
        json: true,
    };
    let plan = Planner::new(registry, install_options).plan()?;

    let mut missing = Vec::new();
    let mut unmanaged = Vec::new();
    let mut stale = Vec::new();

    for file in plan.files {
        if !file.path.exists() {
            missing.push(file.path.display().to_string());
            continue;
        }

        let current = fs::read_to_string(&file.path)?;
        if !current.contains(MANAGED_MARKER) {
            unmanaged.push(file.path.display().to_string());
        } else if current != file.content {
            stale.push(file.path.display().to_string());
        }
    }

    Ok(DoctorReport {
        scope: options.scope.to_string(),
        target: options.target.to_string(),
        ok: missing.is_empty() && unmanaged.is_empty() && stale.is_empty(),
        missing,
        unmanaged,
        stale,
        json: options.json,
    })
}

impl DoctorReport {
    pub fn print(&self) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }

        println!(
            "megara doctor: scope={}, target={}, ok={}",
            self.scope, self.target, self.ok
        );

        print_group("missing", &self.missing);
        print_group("unmanaged", &self.unmanaged);
        print_group("stale", &self.stale);
        Ok(())
    }
}

fn print_group(label: &str, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    println!("{label}:");
    for path in paths {
        println!("- {path}");
    }
}
