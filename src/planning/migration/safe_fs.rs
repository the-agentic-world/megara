use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use uuid::Uuid;

#[path = "safe_fs/at.rs"]
mod at;
#[path = "safe_fs/directories.rs"]
mod directories;
#[path = "safe_fs/iterate.rs"]
mod iterate;
#[path = "safe_fs/remove.rs"]
mod remove;
pub(super) use at::{
    create_directory_at, create_linked_file_at, ensure_directory_at,
    ensure_directory_nofollow_open, open_directory_at, open_directory_nofollow, read_file_at,
    replace_file_at, verify_directory_identity,
};
pub(super) use directories::open_parent_nofollow;
pub(super) use directories::{remove_empty_directory_at, rename_directory_at_noreplace};
#[cfg(not(unix))]
pub(super) use directories::{remove_empty_directory_nofollow, rename_directory_nofollow};
pub(super) use iterate::{open_directory_stream, read_directory_names};
pub(crate) use remove::remove_tree_at_validated;
pub(super) use remove::remove_tree_nofollow;

#[cfg(not(unix))]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::fd::FromRawFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateResult {
    Created,
    Exists,
}

pub(crate) fn temporary_name(base: &str) -> String {
    format!(".{base}.megara-tmp-{}", Uuid::now_v7())
}

pub(crate) fn is_temporary_name(name: &str, base: &str) -> bool {
    name.strip_prefix(&format!(".{base}.megara-tmp-"))
        .is_some_and(|suffix| Uuid::parse_str(suffix).is_ok())
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
pub(super) fn device_id(value: libc::dev_t) -> u64 {
    #[cfg(target_os = "linux")]
    {
        value
    }
    #[cfg(not(target_os = "linux"))]
    {
        value as u64
    }
}

pub(crate) fn read_file_nofollow(path: &Path) -> io::Result<(fs::Metadata, Vec<u8>)> {
    read_file_nofollow_limited(path, usize::MAX)
}

pub(crate) fn read_file_nofollow_limited(
    path: &Path,
    limit: usize,
) -> io::Result<(fs::Metadata, Vec<u8>)> {
    #[cfg(unix)]
    {
        let (parent, name) = open_parent_nofollow(path)?;
        let fd = unsafe {
            libc::openat(
                std::os::fd::AsRawFd::as_raw_fd(&parent),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration path is not a regular file",
            ));
        }
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration file exceeds bounded read limit",
            ));
        }
        Ok((metadata, bytes))
    }
    #[cfg(not(unix))]
    {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration path is not a regular file",
            ));
        }
        let mut file = fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration file exceeds bounded read limit",
            ));
        }
        Ok((metadata, bytes))
    }
}

pub(crate) fn create_linked_file(path: &Path, bytes: &[u8], mode: u32) -> io::Result<CreateResult> {
    #[cfg(unix)]
    {
        let (parent, destination) = open_parent_nofollow(path)?;
        let temporary = std::ffi::CString::new(temporary_name(&destination.to_string_lossy()))
            .map_err(invalid_name)?;
        let fd = unsafe {
            libc::openat(
                std::os::fd::AsRawFd::as_raw_fd(&parent),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        let result = (|| -> io::Result<CreateResult> {
            file.write_all(bytes)?;
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(mode))?;
            file.sync_all()?;
            let linked = unsafe {
                libc::linkat(
                    std::os::fd::AsRawFd::as_raw_fd(&parent),
                    temporary.as_ptr(),
                    std::os::fd::AsRawFd::as_raw_fd(&parent),
                    destination.as_ptr(),
                    0,
                )
            };
            if linked < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::AlreadyExists {
                    return Ok(CreateResult::Exists);
                }
                return Err(error);
            }
            Ok(CreateResult::Created)
        })();
        let removed = unsafe {
            libc::unlinkat(
                std::os::fd::AsRawFd::as_raw_fd(&parent),
                temporary.as_ptr(),
                0,
            )
        };
        if removed < 0 {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()?;
        result
    }
    #[cfg(not(unix))]
    {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
        let metadata = fs::symlink_metadata(parent)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "destination parent is not a directory",
            ));
        }
        let temporary =
            path.with_file_name(temporary_name(&path.file_name().unwrap().to_string_lossy()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if let Err(error) = fs::hard_link(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            if error.kind() == io::ErrorKind::AlreadyExists {
                return Ok(CreateResult::Exists);
            }
            return Err(error);
        }
        fs::remove_file(&temporary)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(CreateResult::Created)
    }
}

#[cfg(not(unix))]
pub(crate) fn replace_file_nofollow(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary =
        path.with_file_name(temporary_name(&path.file_name().unwrap().to_string_lossy()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub(crate) fn remove_file_if_matches_nofollow(
    path: &Path,
    expected_sha256: &str,
    expected_mode: u32,
) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::PermissionsExt;

        let (parent, name) = open_parent_nofollow(path)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Ok(false);
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file()
            || super::inventory::sha256(&{
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                bytes
            }) != expected_sha256
            || metadata.permissions().mode() != expected_mode
        {
            return Ok(false);
        }
        let mut first = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(file.as_raw_fd(), first.as_mut_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut current = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                parent.as_raw_fd(),
                name.as_ptr(),
                current.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            return Ok(false);
        }
        let first = unsafe { first.assume_init() };
        let current = unsafe { current.assume_init() };
        if first.st_dev != current.st_dev || first.st_ino != current.st_ino {
            return Ok(false);
        }
        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } < 0 {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let (metadata, bytes) = read_file_nofollow(path)?;
        if super::inventory::sha256(&bytes) != expected_sha256 {
            return Ok(false);
        }
        let _ = expected_mode;
        fs::remove_file(path)?;
        let _ = metadata;
        Ok(true)
    }
}

pub(crate) fn remove_file_nofollow(path: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        let (parent, name) = open_parent_nofollow(path)?;
        if unsafe { libc::unlinkat(std::os::fd::AsRawFd::as_raw_fd(&parent), name.as_ptr(), 0) } < 0
        {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(false);
            }
            return Err(error);
        }
        parent.sync_all()?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                fs::remove_file(path)?;
                Ok(true)
            }
            Ok(_) => Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn invalid_name(error: std::ffi::NulError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
