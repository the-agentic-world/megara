use std::{io, path::Path};

#[cfg(not(unix))]
use std::fs;

#[cfg(not(unix))]
pub(crate) fn rename_directory_nofollow(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let (source_parent, source_name) = open_parent_nofollow(source)?;
        let (destination_parent, destination_name) = open_parent_nofollow(destination)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        let exists = unsafe {
            libc::fstatat(
                std::os::fd::AsRawFd::as_raw_fd(&destination_parent),
                destination_name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if exists == 0 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "migration destination already exists",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::NotFound {
            return Err(error);
        }
        #[cfg(target_os = "linux")]
        let renamed = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                std::os::fd::AsRawFd::as_raw_fd(&source_parent),
                source_name.as_ptr(),
                std::os::fd::AsRawFd::as_raw_fd(&destination_parent),
                destination_name.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let renamed = unsafe {
            libc::renameatx_np(
                std::os::fd::AsRawFd::as_raw_fd(&source_parent),
                source_name.as_ptr(),
                std::os::fd::AsRawFd::as_raw_fd(&destination_parent),
                destination_name.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace directory publish is unsupported on this platform",
        ));
        if renamed < 0 {
            return Err(io::Error::last_os_error());
        }
        source_parent.sync_all()?;
        destination_parent.sync_all()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "migration destination already exists",
            ));
        }
        fs::rename(source, destination)?;
        if let Some(parent) = source.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        if let Some(parent) = destination.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }
}

#[cfg(unix)]
pub(crate) fn rename_directory_at_noreplace(
    source_parent: &std::fs::File,
    source_name: &std::ffi::CStr,
    source: &std::fs::File,
    destination_parent: &std::fs::File,
    destination_name: &std::ffi::CStr,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    let mut source_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            source_stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let source_stat = unsafe { source_stat.assume_init() };
    if (source_stat.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "migration publish source is not a directory",
        ));
    }
    let held_stat = source.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if super::device_id(source_stat.st_dev) != held_stat.dev()
            || source_stat.st_ino != held_stat.ino()
        {
            return Err(io::Error::other("migration publish source changed"));
        }
    }
    let mut destination_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let exists = unsafe {
        libc::fstatat(
            destination_parent.as_raw_fd(),
            destination_name.as_ptr(),
            destination_stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if exists == 0 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "migration destination already exists",
        ));
    }
    let error = io::Error::last_os_error();
    if error.kind() != io::ErrorKind::NotFound {
        return Err(error);
    }
    #[cfg(target_os = "linux")]
    let renamed = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            destination_parent.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let renamed = unsafe {
        libc::renameatx_np(
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            destination_parent.as_raw_fd(),
            destination_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace directory publish is unsupported on this platform",
    ));
    if renamed < 0 {
        return Err(io::Error::last_os_error());
    }
    source_parent.sync_all()?;
    destination_parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
pub(crate) fn remove_empty_directory_at(
    parent: &std::fs::File,
    name: &std::ffi::CStr,
    directory: &std::fs::File,
) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    let mut path_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            path_stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound {
            return Ok(false);
        }
        return Err(error);
    }
    let path_stat = unsafe { path_stat.assume_init() };
    if (path_stat.st_mode & libc::S_IFMT) != libc::S_IFDIR {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "migration staging namespace is not a directory",
        ));
    }
    let held_stat = directory.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if super::device_id(path_stat.st_dev) != held_stat.dev()
            || path_stat.st_ino != held_stat.ino()
        {
            return Err(io::Error::other("migration staging namespace changed"));
        }
    }
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound
            || error.kind() == io::ErrorKind::DirectoryNotEmpty
        {
            return Ok(false);
        }
        return Err(error);
    }
    parent.sync_all()?;
    Ok(true)
}

#[cfg(not(unix))]
pub(crate) fn remove_empty_directory_nofollow(path: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        let (parent, name) = open_parent_nofollow(path)?;
        if unsafe {
            libc::unlinkat(
                std::os::fd::AsRawFd::as_raw_fd(&parent),
                name.as_ptr(),
                libc::AT_REMOVEDIR,
            )
        } < 0
        {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound
                || error.kind() == io::ErrorKind::DirectoryNotEmpty
            {
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
            Ok(metadata) if metadata.file_type().is_dir() => {
                match fs::remove_dir(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
                        return Ok(false)
                    }
                    Err(error) => return Err(error),
                }
                if let Some(parent) = path.parent() {
                    fs::File::open(parent)?.sync_all()?;
                }
                Ok(true)
            }
            Ok(_) => Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

#[cfg(unix)]
pub(crate) fn open_parent_nofollow(path: &Path) -> io::Result<(std::fs::File, std::ffi::CString)> {
    use std::os::fd::{AsRawFd, FromRawFd};

    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
    let mut directory = std::fs::File::open(if path.is_absolute() {
        Path::new("/")
    } else {
        Path::new(".")
    })?;
    for component in parent.components() {
        let std::path::Component::Normal(part) = component else {
            continue;
        };
        let name = std::ffi::CString::new(part.as_encoded_bytes()).map_err(invalid_name)?;
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
        directory = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    let destination = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing name"))?;
    Ok((
        directory,
        std::ffi::CString::new(destination.as_encoded_bytes()).map_err(invalid_name)?,
    ))
}

fn invalid_name(error: std::ffi::NulError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
