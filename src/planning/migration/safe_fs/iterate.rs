use std::{fs::File, io};

#[cfg(unix)]
pub(crate) fn open_directory_stream(directory: &File) -> io::Result<*mut libc::DIR> {
    let raw_fd = unsafe {
        libc::openat(
            std::os::fd::AsRawFd::as_raw_fd(directory),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if raw_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = unsafe { libc::fdopendir(raw_fd) };
    if stream.is_null() {
        unsafe { libc::close(raw_fd) };
        return Err(io::Error::last_os_error());
    }
    Ok(stream)
}

#[cfg(unix)]
pub(crate) fn read_directory_names(directory: &File) -> io::Result<Vec<std::ffi::CString>> {
    use std::ffi::CStr;

    let stream = open_directory_stream(directory)?;
    let result = (|| -> io::Result<Vec<std::ffi::CString>> {
        let mut names = Vec::new();
        loop {
            reset_errno();
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(0) {
                    return Ok(names);
                }
                return Err(error);
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() != b"." && name.to_bytes() != b".." {
                names.push(
                    std::ffi::CString::new(name.to_bytes())
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
                );
            }
        }
    })();
    let closed = unsafe { libc::closedir(stream) };
    if result.is_ok() && closed < 0 {
        return Err(io::Error::last_os_error());
    }
    result
}

#[cfg(not(unix))]
pub(crate) fn read_directory_names(_directory: &File) -> io::Result<Vec<std::ffi::CString>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative directory enumeration is unsupported",
    ))
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

#[cfg(all(test, unix))]
mod tests {
    use std::{collections::BTreeSet, fs, fs::File};

    use super::read_directory_names;

    #[test]
    fn repeated_enumeration_uses_an_independent_directory_cursor() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first"), b"first").unwrap();
        fs::write(root.path().join("second"), b"second").unwrap();
        let directory = File::open(root.path()).unwrap();

        let names = || {
            read_directory_names(&directory)
                .unwrap()
                .into_iter()
                .map(|name| name.into_bytes())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(names(), names());
    }
}
