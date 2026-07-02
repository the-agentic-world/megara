use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::installer::strip_managed_marker;

use super::{
    model::{HarnessTemplate, TemplateKind},
    specs::TEMPLATE_SPECS,
};

#[derive(Clone, Debug)]
pub struct TemplateRegistry {
    files: Vec<HarnessTemplate>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self {
            files: TEMPLATE_SPECS
                .iter()
                .map(|spec| spec.to_template())
                .collect(),
        }
    }
}

impl TemplateRegistry {
    pub fn from_ssot_root(root: &Path) -> Result<Self> {
        let files = TEMPLATE_SPECS
            .iter()
            .map(|spec| {
                let path = root.join(spec.relative_path);
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read SSOT file {}", path.display()))?;
                let mut template = spec.to_template();
                template.content = strip_managed_marker(&content);
                Ok(template)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { files })
    }

    pub fn missing_paths(root: &Path) -> Vec<PathBuf> {
        TEMPLATE_SPECS
            .iter()
            .map(|spec| root.join(spec.relative_path))
            .filter(|path| !path.exists())
            .collect()
    }

    pub fn ssot_files(&self) -> &[HarnessTemplate] {
        &self.files
    }

    pub fn config(&self) -> Option<&HarnessTemplate> {
        self.by_kind(TemplateKind::Config).into_iter().next()
    }

    pub fn workflows(&self) -> Vec<&HarnessTemplate> {
        self.by_kind(TemplateKind::Workflow)
    }

    pub fn fragments(&self) -> Vec<&HarnessTemplate> {
        self.by_kind(TemplateKind::SkillFragment)
    }

    pub fn agents(&self) -> Vec<&HarnessTemplate> {
        self.by_kind(TemplateKind::Agent)
    }

    pub fn template_names(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|template| template.name.clone())
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&HarnessTemplate> {
        self.files
            .iter()
            .find(|template| template.name == name || template.relative_path == name)
    }

    fn by_kind(&self, kind: TemplateKind) -> Vec<&HarnessTemplate> {
        self.files
            .iter()
            .filter(|template| template.kind == kind)
            .collect()
    }
}
