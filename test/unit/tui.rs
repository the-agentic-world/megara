use crate::{
    cli::{DoctorArgs, InstallArgs, ScopeArg, TargetArg, UpdateArgs, UpdateScopeArg},
    tui::{
        doctor_tui_options, scripted_install_wizard, use_doctor_tui_for, use_install_tui_for,
        use_update_tui_for, TuiInput,
    },
};

fn install_args(scope: Option<ScopeArg>, target: Option<TargetArg>) -> InstallArgs {
    InstallArgs {
        scope,
        target,
        locale: None,
        dry_run: false,
        force: false,
        json: false,
        no_interactive: false,
    }
}

fn doctor_args(scope: Option<ScopeArg>, target: Option<TargetArg>) -> DoctorArgs {
    DoctorArgs {
        scope,
        target,
        json: false,
        no_interactive: false,
    }
}

fn update_args() -> UpdateArgs {
    UpdateArgs {
        scope: UpdateScopeArg::All,
        target: TargetArg::Codex,
        force: false,
        no_interactive: false,
    }
}

#[test]
fn install_tui_only_handles_missing_tty_inputs() {
    let missing = install_args(None, Some(TargetArg::Codex));
    assert!(use_install_tui_for(&missing, true, false));

    let complete = install_args(Some(ScopeArg::Project), Some(TargetArg::Codex));
    assert!(use_install_tui_for(&complete, true, false));

    let mut complete = complete;
    complete.locale = Some("ko-KR".to_string());
    assert!(!use_install_tui_for(&complete, true, false));
    assert!(!use_install_tui_for(&missing, false, false));
    assert!(!use_install_tui_for(&missing, true, true));
}

#[test]
fn install_tui_respects_json_and_no_interactive() {
    let mut args = install_args(None, None);
    args.json = true;
    assert!(!use_install_tui_for(&args, true, false));

    args.json = false;
    args.no_interactive = true;
    assert!(!use_install_tui_for(&args, true, false));
}

#[test]
fn update_tui_requires_tty_and_interactive_mode() {
    let args = update_args();
    assert!(use_update_tui_for(&args, true, false));
    assert!(!use_update_tui_for(&args, false, false));
    assert!(!use_update_tui_for(&args, true, true));

    let mut disabled = update_args();
    disabled.no_interactive = true;
    assert!(!use_update_tui_for(&disabled, true, false));
}

#[test]
fn doctor_tui_only_handles_bare_tty_non_json() {
    let args = doctor_args(None, None);
    assert!(use_doctor_tui_for(&args, true, false));
    assert!(!use_doctor_tui_for(&args, false, false));
    assert!(!use_doctor_tui_for(&args, true, true));

    let with_scope = doctor_args(Some(ScopeArg::Project), None);
    assert!(!use_doctor_tui_for(&with_scope, true, false));

    let mut json = doctor_args(None, None);
    json.json = true;
    assert!(!use_doctor_tui_for(&json, true, false));
}

#[test]
fn doctor_tui_defaults_to_project_codex() {
    let options = doctor_tui_options(doctor_args(None, None)).expect("doctor options");
    assert_eq!(options.scope.to_string(), "project");
    assert_eq!(options.target.to_string(), "codex");
    assert!(!options.json);
}

#[test]
fn scripted_install_wizard_collects_missing_values_and_confirms() {
    let args = install_args(None, None);
    let result = scripted_install_wizard(
        args,
        &[
            TuiInput::Select(1),
            TuiInput::Select(1),
            TuiInput::Confirm,
            TuiInput::Select(0),
        ],
    )
    .expect("wizard should succeed")
    .expect("wizard should confirm");

    assert_eq!(result.locale.as_deref(), Some("en-US"));
    assert_eq!(result.scope, Some(ScopeArg::Global));
    assert_eq!(result.target, Some(TargetArg::Codex));
}

#[test]
fn scripted_install_wizard_can_cancel_before_side_effects() {
    let args = install_args(None, None);
    let result = scripted_install_wizard(args, &[TuiInput::Cancel]).expect("wizard should return");
    assert!(result.is_none());
}

#[test]
fn scripted_install_wizard_preserves_existing_flags() {
    let mut args = install_args(Some(ScopeArg::Project), None);
    args.dry_run = true;
    args.force = true;
    let result = scripted_install_wizard(
        args,
        &[TuiInput::Select(0), TuiInput::Confirm, TuiInput::Confirm],
    )
    .expect("wizard should succeed")
    .expect("wizard should confirm");

    assert_eq!(result.locale.as_deref(), Some("ko-KR"));
    assert_eq!(result.scope, Some(ScopeArg::Project));
    assert_eq!(result.target, Some(TargetArg::Codex));
    assert!(result.dry_run);
    assert!(result.force);
}
