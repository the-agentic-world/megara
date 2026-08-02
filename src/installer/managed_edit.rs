use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct ManagedTomlEdit {
    pub path: PathBuf,
    pub created: bool,
    pub changed: bool,
    pub backup_path: Option<PathBuf>,
    #[serde(skip)]
    pub(crate) desired: String,
    #[serde(skip)]
    pub(crate) backup: Option<Vec<u8>>,
    #[serde(skip)]
    pub(crate) expected_source: Option<Vec<u8>>,
    #[serde(skip)]
    pub(crate) permissions: Option<fs::Permissions>,
}

impl ManagedTomlEdit {
    pub(crate) fn apply(&self, dry_run: bool) -> Result<()> {
        if dry_run || !self.changed {
            return Ok(());
        }
        self.prepare()?.commit()
    }

    pub(crate) fn prepare(&self) -> Result<PreparedManagedTomlEdit<'_>> {
        verify_expected_source(&self.path, self.expected_source.as_deref())?;
        let mut backup_created = false;
        if let (Some(path), Some(bytes)) = (&self.backup_path, &self.backup) {
            let result = (|| -> Result<()> {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .with_context(|| {
                        format!("failed to create managed TOML backup {}", path.display())
                    })?;
                backup_created = true;
                file.write_all(bytes)?;
                file.sync_all()?;
                Ok(())
            })();
            if let Err(error) = result {
                if backup_created {
                    let _ = fs::remove_file(path);
                }
                return Err(error);
            }
        }

        let parent = self.path.parent().context("managed TOML has no parent")?;
        fs::create_dir_all(parent)?;
        let file_name = self
            .path
            .file_name()
            .context("managed TOML has no file name")?
            .to_string_lossy();
        let temporary = parent.join(format!(".{file_name}.tmp-{}", Uuid::now_v7()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(self.desired.as_bytes())?;
            if let Some(permissions) = &self.permissions {
                fs::set_permissions(&temporary, permissions.clone())?;
            }
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            if backup_created {
                if let Some(path) = &self.backup_path {
                    let _ = fs::remove_file(path);
                }
            }
            return Err(error);
        }
        Ok(PreparedManagedTomlEdit {
            edit: self,
            temporary,
            backup_created,
            committed: false,
        })
    }
}

pub(crate) struct PreparedManagedTomlEdit<'a> {
    edit: &'a ManagedTomlEdit,
    temporary: PathBuf,
    backup_created: bool,
    committed: bool,
}

impl PreparedManagedTomlEdit<'_> {
    pub(crate) fn commit(mut self) -> Result<()> {
        verify_expected_source(&self.edit.path, self.edit.expected_source.as_deref())?;
        fs::rename(&self.temporary, &self.edit.path)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for PreparedManagedTomlEdit<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let _ = fs::remove_file(&self.temporary);
        if self.backup_created {
            if let Some(path) = &self.edit.backup_path {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn verify_expected_source(path: &Path, expected: Option<&[u8]>) -> Result<()> {
    match (fs::read(path), expected) {
        (Ok(actual), Some(expected)) if actual == expected => Ok(()),
        (Ok(_), Some(_)) => anyhow::bail!(
            "PROJECTION_DIVERGED: managed TOML changed after planning: {}",
            path.display()
        ),
        (Ok(_), None) => anyhow::bail!(
            "PROJECTION_DIVERGED: managed TOML appeared after planning: {}",
            path.display()
        ),
        (Err(error), Some(_)) if error.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!(
                "PROJECTION_DIVERGED: managed TOML was removed after planning: {}",
                path.display()
            )
        }
        (Err(error), None) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        (Err(error), _) => {
            Err(error).with_context(|| format!("failed to reread managed TOML {}", path.display()))
        }
    }
}
