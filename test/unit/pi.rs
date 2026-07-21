use std::path::PathBuf;

use super::*;

#[test]
fn pi_target_projects_settings_extension_and_role_agents() {
    let files = targets::pi::projection_files(
        PathBuf::from(".pi"),
        paths::InstallScope::Project,
        &templates::TemplateRegistry::default(),
    )
    .unwrap();
    let paths = files
        .iter()
        .map(|file| file.path.display().to_string())
        .collect::<Vec<_>>();

    assert!(paths.iter().any(|path| path.ends_with(".pi/settings.json")));
    assert!(paths
        .iter()
        .any(|path| path.ends_with(".pi/extensions/megara.ts")));
    assert!(paths
        .iter()
        .any(|path| path.ends_with(".pi/agents/executor.md")));
}

#[test]
fn targets_list_includes_pi_as_supported() {
    assert!(targets::supported_targets()
        .iter()
        .any(|target| target.name == "pi" && target.status == "supported"));
}
