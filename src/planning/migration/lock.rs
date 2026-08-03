use std::{
    fs::File,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

pub(crate) struct MigrationLock {
    _directory: File,
}

pub(crate) fn acquire(project: &Path) -> Result<MigrationLock> {
    let directory = File::open(project).with_context(|| {
        format!(
            "failed to open project lock directory {}",
            project.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result =
                unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result == 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK)
                && error.raw_os_error() != Some(libc::EAGAIN)
            {
                return Err(error.into());
            }
            if Instant::now() >= deadline {
                anyhow::bail!("MIGRATION_BUSY: project migration lock is held")
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("migration locks unsupported on this platform")
    }
    Ok(MigrationLock {
        _directory: directory,
    })
}
