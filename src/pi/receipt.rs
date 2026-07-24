use std::{
    fs,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const LOCK_WAIT: Duration = Duration::from_secs(5);
const STALE_LOCK_AGE: Duration = Duration::from_secs(30);
const LOCK_RETRY: Duration = Duration::from_millis(10);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EventReceipt {
    pub event_id: String,
    pub workflow: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub attempts: Vec<AttemptReceipt>,
    #[serde(default)]
    pub fallback_attempted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttemptReceipt {
    pub id: String,
    pub role: String,
    pub status: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

pub fn path_for(runtime_root: &Path, event_id: &str) -> PathBuf {
    runtime_root
        .join("state/workflows/pi/events")
        .join(format!("{event_id}.json"))
}

pub fn with_lock<T>(
    runtime_root: &Path,
    event_id: &str,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let receipt_path = path_for(runtime_root, event_id);
    let parent = receipt_path.parent().expect("receipt path has parent");
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let lock = receipt_path.with_extension("json.lock");
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_stale_lock(&lock) {
                    let _ = fs::remove_file(&lock);
                    continue;
                }
                if Instant::now() >= deadline {
                    return Err(error)
                        .with_context(|| format!("timed out waiting for {}", lock.display()));
                }
                thread::sleep(LOCK_RETRY);
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to lock {}", lock.display()))
            }
        }
    }
    let result = operation();
    let _ = fs::remove_file(&lock);
    result
}

pub fn load(runtime_root: &Path, event_id: &str) -> Result<Option<EventReceipt>> {
    let path = path_for(runtime_root, event_id);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(Some(serde_json::from_str(&content).with_context(|| {
        format!("failed to parse {}", path.display())
    })?))
}

pub fn save(runtime_root: &Path, receipt: &EventReceipt) -> Result<()> {
    let path = path_for(runtime_root, &receipt.event_id);
    let parent = path.parent().expect("receipt path has parent");
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let temporary = path.with_extension(format!(
        "json.{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temporary, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn is_stale_lock(path: &Path) -> bool {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .and_then(|modified| {
            SystemTime::now()
                .duration_since(modified)
                .map_err(std::io::Error::other)
        })
        .is_ok_and(|age| age >= STALE_LOCK_AGE)
}
