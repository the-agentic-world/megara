use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use super::CreateResult;

pub(crate) fn ensure_directory_nofollow_open(path: &Path) -> io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let mut directory = fs::File::open(if path.is_absolute() {
            Path::new("/")
        } else {
            Path::new(".")
        })?;
        for component in path.components() {
            let std::path::Component::Normal(part) = component else {
                continue;
            };
            let name = c_name(part)?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if fd < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
                if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                directory.sync_all()?;
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                directory = unsafe { fs::File::from_raw_fd(fd) };
            } else {
                directory = unsafe { fs::File::from_raw_fd(fd) };
            }
        }
        directory.sync_all()?;
        Ok(directory)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)?;
        let directory = fs::File::open(path)?;
        directory.sync_all()?;
        Ok(directory)
    }
}

pub(crate) fn open_directory_nofollow(path: &Path) -> io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let mut directory = fs::File::open(if path.is_absolute() {
            Path::new("/")
        } else {
            Path::new(".")
        })?;
        for component in path.components() {
            let std::path::Component::Normal(part) = component else {
                continue;
            };
            let name = c_name(part)?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            directory = unsafe { fs::File::from_raw_fd(fd) };
        }
        Ok(directory)
    }
    #[cfg(not(unix))]
    {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration path is not a directory",
            ));
        }
        fs::File::open(path)
    }
}

pub(crate) fn open_directory_at(parent: &fs::File, name: &std::ffi::CStr) -> io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { fs::File::from_raw_fd(fd) })
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, name);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative directory open is unsupported",
        ))
    }
}

pub(crate) fn create_directory_at(
    parent: &fs::File,
    name: &str,
    mode: u32,
) -> io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let name = std::ffi::CString::new(name).map_err(invalid_name)?;
        let mode = checked_mode(mode)?;
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), mode) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::fchmod(fd, mode) } < 0 {
            let error = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(error);
        }
        let directory = unsafe { fs::File::from_raw_fd(fd) };
        directory.sync_all()?;
        parent.sync_all()?;
        Ok(directory)
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, name, mode);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative directory creation is unsupported",
        ))
    }
}

pub(crate) fn ensure_directory_at(parent: &fs::File, relative: &Path) -> io::Result<fs::File> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let mut directory = parent.try_clone()?;
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                continue;
            };
            let name = c_name(part)?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if fd < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
                if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                directory.sync_all()?;
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                directory = unsafe { fs::File::from_raw_fd(fd) };
            } else {
                directory = unsafe { fs::File::from_raw_fd(fd) };
            }
        }
        directory.sync_all()?;
        Ok(directory)
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, relative);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative directory traversal is unsupported",
        ))
    }
}

pub(crate) fn create_linked_file_at(
    parent: &fs::File,
    destination: &std::ffi::CStr,
    bytes: &[u8],
    mode: u32,
) -> io::Result<CreateResult> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::PermissionsExt;

        let temporary =
            std::ffi::CString::new(super::temporary_name(&destination.to_string_lossy()))
                .map_err(invalid_name)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { fs::File::from_raw_fd(fd) };
        let result = (|| -> io::Result<CreateResult> {
            file.write_all(bytes)?;
            file.set_permissions(fs::Permissions::from_mode(mode))?;
            file.sync_all()?;
            if unsafe {
                libc::linkat(
                    parent.as_raw_fd(),
                    temporary.as_ptr(),
                    parent.as_raw_fd(),
                    destination.as_ptr(),
                    0,
                )
            } < 0
            {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::AlreadyExists {
                    return Ok(CreateResult::Exists);
                }
                return Err(error);
            }
            Ok(CreateResult::Created)
        })();
        let removed = unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) };
        if removed < 0 && io::Error::last_os_error().kind() != io::ErrorKind::NotFound {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()?;
        result
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, destination, bytes, mode);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative file creation is unsupported",
        ))
    }
}

pub(crate) fn read_file_at(
    parent: &fs::File,
    name: &std::ffi::CStr,
    limit: usize,
) -> io::Result<(fs::Metadata, Vec<u8>)> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};

        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { fs::File::from_raw_fd(fd) };
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
        let _ = (parent, name, limit);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative file reads are unsupported",
        ))
    }
}

pub(crate) fn replace_file_at(
    parent: &fs::File,
    destination: &std::ffi::CStr,
    bytes: &[u8],
) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::PermissionsExt;

        let temporary =
            std::ffi::CString::new(super::temporary_name(&destination.to_string_lossy()))
                .map_err(invalid_name)?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { fs::File::from_raw_fd(fd) };
        let result = (|| -> io::Result<()> {
            file.write_all(bytes)?;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.sync_all()?;
            if unsafe {
                libc::renameat(
                    parent.as_raw_fd(),
                    temporary.as_ptr(),
                    parent.as_raw_fd(),
                    destination.as_ptr(),
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            parent.sync_all()
        })();
        if result.is_err() {
            let removed = unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) };
            if removed < 0 && io::Error::last_os_error().kind() != io::ErrorKind::NotFound {
                return Err(io::Error::last_os_error());
            }
            parent.sync_all()?;
        }
        result
    }
    #[cfg(not(unix))]
    {
        let _ = (parent, destination, bytes);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative file replacement is unsupported",
        ))
    }
}

pub(crate) fn verify_directory_identity(path: &Path, directory: &fs::File) -> io::Result<()> {
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "migration directory is not a directory",
        ));
    }
    let held_metadata = directory.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != held_metadata.dev() || path_metadata.ino() != held_metadata.ino()
        {
            return Err(io::Error::other("migration directory changed"));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn c_name(part: &std::ffi::OsStr) -> io::Result<std::ffi::CString> {
    std::ffi::CString::new(part.as_encoded_bytes()).map_err(invalid_name)
}

#[cfg(unix)]
fn checked_mode(mode: u32) -> io::Result<libc::mode_t> {
    libc::mode_t::try_from(mode)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "directory mode is too large"))
}

fn invalid_name(error: std::ffi::NulError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
