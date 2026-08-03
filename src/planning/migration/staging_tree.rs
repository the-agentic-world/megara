use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

use super::StagingFile;

#[cfg(unix)]
pub(super) fn validate_tree_at(
    root: &fs::File,
    files: &BTreeMap<String, &StagingFile>,
    complete: bool,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    walk_directory_at(root, Path::new(""), true, files, complete, seen)
}

#[cfg(not(unix))]
pub(super) fn validate_tree_at(
    _root: &fs::File,
    _files: &BTreeMap<String, &StagingFile>,
    _complete: bool,
    _seen: &mut BTreeSet<String>,
) -> Result<()> {
    Err(anyhow!(
        "descriptor-relative staging validation is unsupported"
    ))
}

#[cfg(unix)]
fn walk_directory_at(
    directory: &fs::File,
    relative: &Path,
    root: bool,
    files: &BTreeMap<String, &StagingFile>,
    complete: bool,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    use std::ffi::CStr;

    let directory_stream = super::super::safe_fs::open_directory_stream(directory)?;
    let directory_fd = unsafe { libc::dirfd(directory_stream) };
    let result = (|| -> Result<()> {
        loop {
            reset_errno();
            let entry = unsafe { libc::readdir(directory_stream) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(0) {
                    break;
                }
                return Err(error.into());
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            let name_text = name
                .to_str()
                .map_err(|_| anyhow!("staging entry name is not UTF-8"))?;
            let child_relative = if relative.as_os_str().is_empty() {
                PathBuf::from(name_text)
            } else {
                relative.join(name_text)
            };
            let stat = stat_at(directory_fd, name)?.ok_or_else(|| {
                anyhow!("staging entry disappeared: {}", child_relative.display())
            })?;
            if (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR {
                if root && name_text != "files" {
                    return Err(anyhow!(
                        "unexpected staging directory: {}",
                        child_relative.display()
                    ));
                }
                if !root
                    && !files.keys().any(|expected| {
                        let expected = Path::new(expected);
                        expected != child_relative && expected.starts_with(&child_relative)
                    })
                {
                    return Err(anyhow!(
                        "unexpected staging directory: {}",
                        child_relative.display()
                    ));
                }
                let child = super::super::safe_fs::open_directory_at(
                    directory,
                    &std::ffi::CString::new(name.to_bytes())?,
                )?;
                if !opened_matches_stat(&stat, &child.metadata()?)? {
                    return Err(anyhow!(
                        "staging directory changed during validation: {}",
                        child_relative.display()
                    ));
                }
                if root {
                    walk_directory_at(&child, Path::new(""), false, files, complete, seen)?;
                } else {
                    walk_directory_at(&child, &child_relative, false, files, complete, seen)?;
                }
                continue;
            }
            if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG {
                return Err(anyhow!(
                    "unexpected staging entry: {}",
                    child_relative.display()
                ));
            }
            if root {
                if name_text == "staging.json" || (complete && name_text == "manifest.json") {
                    continue;
                }
                return Err(anyhow!(
                    "unexpected staging entry: {}",
                    child_relative.display()
                ));
            }
            if let Some(expected) = files.get(&child_relative.to_string_lossy().to_string()) {
                verify_file(directory, name, expected, &child_relative, false, &stat)?;
                seen.insert(child_relative.to_string_lossy().into_owned());
            } else if complete {
                return Err(anyhow!(
                    "unexpected staging backup file: {}",
                    child_relative.display()
                ));
            } else if let Some(expected) = temp_expected(&child_relative, name_text, files) {
                verify_file(directory, name, expected, &child_relative, true, &stat)?;
            } else {
                return Err(anyhow!(
                    "unexpected staging backup file: {}",
                    child_relative.display()
                ));
            }
        }
        Ok(())
    })();
    let closed = unsafe { libc::closedir(directory_stream) };
    if result.is_ok() && closed < 0 {
        return Err(io::Error::last_os_error().into());
    }
    result
}

#[cfg(unix)]
fn verify_file(
    directory: &fs::File,
    name: &std::ffi::CStr,
    expected: &StagingFile,
    relative: &Path,
    temporary: bool,
    expected_stat: &libc::stat,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let (metadata, bytes) = super::super::safe_fs::read_file_at(
        directory,
        name,
        usize::try_from(expected.size).map_err(|_| anyhow!("staging file size is too large"))?,
    )?;
    if !opened_matches_stat(expected_stat, &metadata)? {
        return Err(anyhow!(
            "staging file changed during validation: {}",
            relative.display()
        ));
    }
    if metadata.len() != expected.size || super::super::inventory::sha256(&bytes) != expected.sha256
    {
        return Err(anyhow!(
            "staging backup {} digest mismatch: {}",
            if temporary { "temp" } else { "file" },
            relative.display()
        ));
    }
    if metadata.permissions().mode() != expected.mode {
        return Err(anyhow!(
            "staging backup {} mode mismatch: {}",
            if temporary { "temp" } else { "file" },
            relative.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn opened_matches_stat(stat: &libc::stat, metadata: &fs::Metadata) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(
        super::super::safe_fs::device_id(stat.st_dev) == metadata.dev()
            && stat.st_ino == metadata.ino(),
    )
}

#[cfg(unix)]
fn temp_expected<'a>(
    relative: &Path,
    name: &str,
    files: &'a BTreeMap<String, &'a StagingFile>,
) -> Option<&'a StagingFile> {
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    files.iter().find_map(|(expected_path, expected)| {
        let expected_path = Path::new(expected_path);
        let expected_parent = expected_path.parent().unwrap_or_else(|| Path::new(""));
        let base = expected_path.file_name()?.to_str()?;
        (parent == expected_parent && super::super::safe_fs::is_temporary_name(name, base))
            .then_some(*expected)
    })
}

#[cfg(unix)]
fn stat_at(parent: std::os::fd::RawFd, name: &std::ffi::CStr) -> io::Result<Option<libc::stat>> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error);
    }
    Ok(Some(unsafe { stat.assume_init() }))
}

#[cfg(unix)]
fn reset_errno() {
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = 0
    }
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = 0
    }
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    {}
}
