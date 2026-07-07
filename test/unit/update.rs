use super::*;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[test]
fn compares_release_tags() {
    assert!(update::is_newer_tag("v1.0.1", "v1.0.0"));
    assert!(update::is_newer_tag("v1.1.0", "v1.0.9"));
    assert!(!update::is_newer_tag("v1.0.0", "v1.0.0"));
    assert!(!update::is_newer_tag("v0.0.13", "v1.0.0"));
    assert!(!update::is_newer_tag("not-a-version", "v1.0.0"));
}

#[test]
fn explicit_install_dir_wins() {
    let (dir, messages) = update::choose_install_dir(
        Some(PathBuf::from("/tmp/megara-bin")),
        PathBuf::from("/usr/local/bin"),
        true,
        Some(PathBuf::from("/home/user/.local/bin")),
        None,
    )
    .unwrap();
    assert_eq!(dir, PathBuf::from("/tmp/megara-bin"));
    assert_eq!(messages, vec!["Using MEGARA_INSTALL_DIR=/tmp/megara-bin"]);
}

#[test]
fn writable_current_install_dir_is_reused() {
    let (dir, messages) = update::choose_install_dir(
        None,
        PathBuf::from("/opt/megara/bin"),
        true,
        Some(PathBuf::from("/home/user/.local/bin")),
        None,
    )
    .unwrap();
    assert_eq!(dir, PathBuf::from("/opt/megara/bin"));
    assert!(messages.is_empty());
}

#[test]
fn non_writable_current_install_dir_falls_back_to_user_bin() {
    let (dir, messages) = update::choose_install_dir(
        None,
        PathBuf::from("/usr/local/bin"),
        false,
        Some(PathBuf::from("/home/user/.local/bin")),
        None,
    )
    .unwrap();
    assert_eq!(dir, PathBuf::from("/home/user/.local/bin"));
    assert!(messages
        .iter()
        .any(|message| message.contains("Current install dir is not writable")));
    assert!(messages
        .iter()
        .any(|message| message.contains("Add /home/user/.local/bin to PATH")));
}

#[test]
fn fallback_path_message_changes_when_user_bin_is_on_path() {
    let (dir, messages) = update::choose_install_dir(
        None,
        PathBuf::from("/usr/local/bin"),
        false,
        Some(PathBuf::from("/home/user/.local/bin")),
        Some(OsStr::new("/home/user/.local/bin:/usr/local/bin")),
    )
    .unwrap();
    assert_eq!(dir, PathBuf::from("/home/user/.local/bin"));
    assert!(messages
        .iter()
        .any(|message| message.contains("appears before /usr/local/bin")));
}

#[test]
fn non_writable_current_install_dir_can_use_secondary_user_bin() {
    let (dir, messages) = update::choose_install_dir(
        None,
        PathBuf::from("/usr/local/bin"),
        false,
        Some(PathBuf::from("/home/user/bin")),
        None,
    )
    .unwrap();
    assert_eq!(dir, PathBuf::from("/home/user/bin"));
    assert!(messages
        .iter()
        .any(|message| message.contains("installing binary to /home/user/bin")));
}

#[test]
fn non_writable_current_install_dir_errors_without_user_fallback() {
    let err = update::choose_install_dir(None, PathBuf::from("/usr/local/bin"), false, None, None)
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("no writable install directory found"));
}

#[test]
fn legacy_installer_binary_is_cleaned_when_installing_elsewhere() {
    assert!(update::should_cleanup_legacy_binary(
        Path::new("/usr/local/bin/megara"),
        Path::new("/home/user/.local/bin/megara"),
    ));
}

#[test]
fn legacy_installer_binary_is_not_cleaned_when_reused() {
    assert!(!update::should_cleanup_legacy_binary(
        Path::new("/usr/local/bin/megara"),
        Path::new("/usr/local/bin/megara"),
    ));
}

#[test]
fn non_legacy_binary_is_not_cleaned() {
    assert!(!update::should_cleanup_legacy_binary(
        Path::new("/opt/homebrew/bin/megara"),
        Path::new("/home/user/.local/bin/megara"),
    ));
}
