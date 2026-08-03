use std::{io, path::Path};

#[cfg(not(unix))]
use std::fs;

#[cfg(unix)]
use std::{
    ffi::CStr,
    fs::File,
    os::fd::{AsRawFd, FromRawFd, RawFd},
};

#[cfg(unix)]
pub(crate) fn remove_tree_nofollow(path: &Path) -> io::Result<bool> {
    let (parent, name) = super::directories::open_parent_nofollow(path)?;
    let root_fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if root_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let root_file = unsafe { File::from_raw_fd(root_fd) };
    remove_tree_at(&parent, &name, &root_file)
}

#[cfg(unix)]
pub(crate) fn remove_tree_at(parent: &File, name: &CStr, root_file: &File) -> io::Result<bool> {
    remove_tree_at_validated(parent, name, root_file, |_| Ok(()))
}

#[cfg(unix)]
pub(crate) fn remove_tree_at_validated<F>(
    parent: &File,
    name: &CStr,
    root_file: &File,
    validate_after_quarantine: F,
) -> io::Result<bool>
where
    F: FnOnce(&File) -> io::Result<()>,
{
    let root_stat = match stat_at(parent.as_raw_fd(), name)? {
        Some(stat) => stat,
        None => return Ok(false),
    };
    if !is_directory(&root_stat) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "migration cleanup root is not a directory",
        ));
    }
    if !same_stat(&root_stat, &stat_file(root_file)?) {
        return Err(io::Error::other(
            "migration cleanup root changed during preflight",
        ));
    }
    let quarantine = quarantine_entry(parent.as_raw_fd(), name, &root_stat, Some(root_file))?;
    if let Err(error) = validate_after_quarantine(root_file) {
        restore_quarantine(parent.as_raw_fd(), &quarantine, name)?;
        return Err(error);
    }
    let result = remove_directory_fd(root_file.try_clone()?);
    if let Err(error) = result {
        restore_quarantine(parent.as_raw_fd(), &quarantine, name)?;
        return Err(error);
    }
    if unsafe { libc::unlinkat(parent.as_raw_fd(), quarantine.as_ptr(), libc::AT_REMOVEDIR) } < 0 {
        let error = io::Error::last_os_error();
        restore_quarantine(parent.as_raw_fd(), &quarantine, name)?;
        return Err(error);
    }
    parent.sync_all()?;
    Ok(true)
}

#[cfg(not(unix))]
pub(crate) fn remove_tree_at(
    _parent: &fs::File,
    _name: &std::ffi::CStr,
    _root_file: &fs::File,
) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative tree removal is unsupported",
    ))
}

#[cfg(not(unix))]
pub(crate) fn remove_tree_at_validated<F>(
    _parent: &fs::File,
    _name: &std::ffi::CStr,
    _root_file: &fs::File,
    _validate_after_quarantine: F,
) -> io::Result<bool>
where
    F: FnOnce(&fs::File) -> io::Result<()>,
{
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative tree removal is unsupported",
    ))
}

#[cfg(not(unix))]
pub(crate) fn remove_tree_nofollow(path: &Path) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "migration cleanup target is not a directory",
        ));
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let child_metadata = fs::symlink_metadata(&child)?;
        if child_metadata.file_type().is_dir() {
            remove_tree_nofollow(&child)?;
        } else {
            fs::remove_file(&child)?;
        }
    }
    fs::remove_dir(path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(true)
}

#[cfg(unix)]
fn remove_directory_fd(directory: File) -> io::Result<()> {
    use std::ffi::CStr;

    let directory = super::iterate::open_directory_stream(&directory)?;
    let directory_fd = unsafe { libc::dirfd(directory) };
    let result = (|| -> io::Result<()> {
        let mut names = Vec::new();
        loop {
            reset_errno();
            let entry = unsafe { libc::readdir(directory) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(0) {
                    break;
                }
                return Err(error);
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            names.push(std::ffi::CString::new(name.to_bytes()).map_err(invalid_name)?);
        }
        for name in names {
            let child_stat = match stat_at(directory_fd, &name)? {
                Some(stat) => stat,
                None => continue,
            };
            if is_directory(&child_stat) {
                let child_fd = unsafe {
                    libc::openat(
                        directory_fd,
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                    )
                };
                if child_fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                let child_file = unsafe { File::from_raw_fd(child_fd) };
                if !same_stat(&child_stat, &stat_file(&child_file)?) {
                    return Err(io::Error::other(
                        "migration cleanup directory changed during preflight",
                    ));
                }
                let quarantine =
                    quarantine_entry(directory_fd, &name, &child_stat, Some(&child_file))?;
                let result = remove_directory_fd(child_file);
                if let Err(error) = result {
                    restore_quarantine(directory_fd, &quarantine, &name)?;
                    return Err(error);
                }
                if unsafe { libc::unlinkat(directory_fd, quarantine.as_ptr(), libc::AT_REMOVEDIR) }
                    < 0
                {
                    let error = io::Error::last_os_error();
                    restore_quarantine(directory_fd, &quarantine, &name)?;
                    return Err(error);
                }
                sync_directory_fd(directory_fd)?;
            } else {
                let quarantine = quarantine_entry(directory_fd, &name, &child_stat, None)?;
                if unsafe { libc::unlinkat(directory_fd, quarantine.as_ptr(), 0) } < 0 {
                    let error = io::Error::last_os_error();
                    restore_quarantine(directory_fd, &quarantine, &name)?;
                    return Err(error);
                }
                sync_directory_fd(directory_fd)?;
            }
        }
        Ok(())
    })();
    let closed = unsafe { libc::closedir(directory) };
    if result.is_ok() && closed < 0 {
        return Err(io::Error::last_os_error());
    }
    result
}

#[cfg(unix)]
fn quarantine_entry(
    parent: RawFd,
    name: &CStr,
    expected: &libc::stat,
    held: Option<&File>,
) -> io::Result<std::ffi::CString> {
    let quarantine = std::ffi::CString::new(super::temporary_name(&name.to_string_lossy()))
        .map_err(invalid_name)?;
    rename_noreplace(parent, name, parent, &quarantine)?;
    if let Err(error) = sync_directory_fd(parent) {
        let _ = restore_quarantine(parent, &quarantine, name);
        return Err(error);
    }
    let actual = stat_at(parent, &quarantine)?
        .ok_or_else(|| io::Error::other("migration cleanup quarantine disappeared"))?;
    let expected = held.map(stat_file).transpose()?.unwrap_or(*expected);
    if !same_stat(&expected, &actual) {
        restore_quarantine(parent, &quarantine, name)?;
        return Err(io::Error::other("migration cleanup entry changed"));
    }
    Ok(quarantine)
}

#[cfg(unix)]
fn restore_quarantine(parent: RawFd, quarantine: &CStr, name: &CStr) -> io::Result<()> {
    rename_noreplace(parent, quarantine, parent, name)?;
    sync_directory_fd(parent)
}

#[cfg(unix)]
fn rename_noreplace(
    source_parent: RawFd,
    source: &CStr,
    destination_parent: RawFd,
    destination: &CStr,
) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            source_parent,
            source.as_ptr(),
            destination_parent,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            source_parent,
            source.as_ptr(),
            destination_parent,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace cleanup quarantine is unsupported",
    ));
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn stat_at(parent: RawFd, name: &CStr) -> io::Result<Option<libc::stat>> {
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
fn stat_file(file: &File) -> io::Result<libc::stat> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { stat.assume_init() })
}

#[cfg(unix)]
fn same_stat(left: &libc::stat, right: &libc::stat) -> bool {
    left.st_dev == right.st_dev && left.st_ino == right.st_ino
}

#[cfg(unix)]
fn is_directory(stat: &libc::stat) -> bool {
    (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR
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

#[cfg(unix)]
fn sync_directory_fd(fd: RawFd) -> io::Result<()> {
    if unsafe { libc::fsync(fd) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn invalid_name(error: std::ffi::NulError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
