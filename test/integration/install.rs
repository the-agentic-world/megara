use super::*;

#[test]
fn installs_project_scope_codex_harness() {
    let dir = tempdir().unwrap();
    let codex_home = tempdir().unwrap();

    let output = megara_with_codex_home(codex_home.path())
        .arg("install")
        .arg("--scope")
        .arg("project")
        .arg("--target")
        .arg("codex")
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("open a new session after install"));
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(dir.path().join(".agents/bin/megara").exists());
    assert!(dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir
        .path()
        .join(".agents/skills/deep-interview/SKILL.md")
        .exists());
    assert!(dir.path().join(".agents/skills/caveman/SKILL.md").exists());
    assert!(!dir
        .path()
        .join(".codex/skills/deep-interview/SKILL.md")
        .exists());
    assert!(!dir.path().join(".codex/skills/caveman/SKILL.md").exists());
    assert!(dir
        .path()
        .join(".agents/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir
        .path()
        .join(".codex/skill-fragments/deep-interview/auto-research-greenfield.md")
        .exists());
    assert!(dir.path().join(".agents/agents/executor.toml").exists());
    assert!(dir.path().join(".codex/hooks.json").exists());
    assert!(dir.path().join(".codex/agents/executor.toml").exists());
    let skill =
        fs::read_to_string(dir.path().join(".agents/skills/deep-interview/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\n"));
    assert!(skill.contains("MEGARA:MANAGED"));
    assert!(skill.contains("Do not print a separate threshold line"));
    assert!(skill.contains("Default ladder: `15% -> 5% -> 2% -> 0% remaining ambiguity`"));
    assert!(skill.contains("Ambiguity Target Ladder"));
    assert!(skill.contains("15% -> 5% -> 2% -> 0%"));
    assert!(skill.contains("At `15%`, stop asking ordinary interview questions"));
    assert!(skill.contains("At `5%`, stop asking ordinary interview questions"));
    assert!(skill.contains("At `2%`, stop asking ordinary interview questions"));
    assert!(skill.contains("At `0%`, do not ask another milestone decision"));
    assert!(skill.contains("Continue deep-interview to the next ambiguity target"));
    assert!(skill.contains("reaching the active target opens the milestone decision step"));
    assert!(skill.contains("Codex Plan-Mode Preflight"));
    assert!(
        skill.contains("The preflight question must have exactly three visible numbered options")
    );
    assert!(skill.contains("Restart with `/plan <same request>`"));
    assert!(skill.contains("Continue here without `/plan`"));
    assert!(!skill.contains("Continue here, but keep questions extra compact"));
    assert!(skill.contains("begin Round 0 in the next assistant turn using the original request"));
    assert!(skill.contains("<configured-locale ambiguity label>: NN%"));
    assert!(skill.contains("Calculate ambiguity as `100 - weighted_clarity`"));
    assert!(skill.contains("Ambiguity is bidirectional and non-monotonic"));
    assert!(skill.contains("Compact Visible Output"));
    assert!(skill.contains("Keep active interview output compact for humans"));
    assert!(skill.contains("Show the current ambiguity score on every active interview question"));
    assert!(skill.contains("exactly four visible options"));
    assert!(skill
        .contains("The user only needs the ambiguity score, next question, and answer choices"));
    assert!(skill.contains("do not include technical hook blocks"));
    assert!(skill.contains("short numbered visible option list"));
    assert!(skill.contains("1. <option 1>"));
    assert!(skill.contains("3. <option 3>"));
    assert!(skill.contains("4. <configured-locale direct input / not in listed options>"));
    assert!(skill.contains("user may answer with the option number"));
    assert!(skill.contains("direct input / not in listed options"));
    assert!(skill.contains("Do not include technical gate blocks"));
    assert!(skill.contains("Runtime hooks infer the pending question"));
    assert!(skill.contains("Do not emit a visible ledger update"));
    assert!(skill.contains("hidden `Megara Workflow State` comment"));
    assert!(skill.contains("Produce a user-friendly pending-approval summary"));
    assert!(skill.contains("Do not show raw labels such as `Metadata`"));
    assert!(!skill.contains("Interview ledger update:"));
    assert!(!skill.contains("Megara Question Gate:"));
    assert!(skill.contains("Megara Workflow State:"));
    assert!(skill.contains("locked markdown artifact"));
    assert!(skill.contains("spec_path"));
    assert!(skill.contains("Write every user-facing sentence in the configured locale"));
    assert!(skill.contains("option labels"));
    assert!(skill.contains("free-text values"));
    assert!(skill.contains("Do not copy English section headings"));
    assert!(skill.contains("Round 0: Topology Confirmation"));
    assert!(!skill.contains("Deep Interview threshold:"));
    assert!(!skill.contains("I'm reading this as"));
    assert!(!skill.contains("Restate gate"));
    let caveman = fs::read_to_string(dir.path().join(".agents/skills/caveman/SKILL.md")).unwrap();
    assert!(caveman.contains("ACTIVE EVERY RESPONSE"));
    assert!(caveman.contains("stop caveman"));
    let ralplan = fs::read_to_string(dir.path().join(".agents/skills/ralplan/SKILL.md")).unwrap();
    assert!(ralplan.contains("Megara Review Pass:"));
    assert!(ralplan.contains("Megara Plan Gate:"));
    assert!(ralplan.contains("Megara Approval Gate:"));
    assert!(ralplan.contains("hidden runtime metadata comments"));
    assert!(ralplan.contains("Do not show these metadata blocks in visible prose"));
    assert!(ralplan.contains("Normal user approval should be a number or natural-language choice"));
    assert!(ralplan.contains("input_spec_sha256"));
    assert!(ralplan.contains("plan_sha256"));
    assert!(ralplan.contains("pending_approval"));
    let ultragoal =
        fs::read_to_string(dir.path().join(".agents/skills/ultragoal/SKILL.md")).unwrap();
    assert!(ultragoal.contains(r#"MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}""#));
    assert!(ultragoal.contains(r#""$MEGARA_BIN" ultragoal"#));
    assert!(ultragoal.contains("include hidden metadata"));
    assert!(ultragoal.contains("Do not show `Megara Workflow State`"));
    assert!(!ultragoal.contains("\nmegara ultragoal"));
    assert!(megara_with_codex_home(codex_home.path())
        .arg("--version")
        .current_dir(dir.path())
        .output()
        .unwrap()
        .status
        .success());
    let wrapper = dir.path().join(".agents/bin/megara");
    assert!(Command::new(&wrapper)
        .arg("--version")
        .current_dir(dir.path())
        .output()
        .unwrap()
        .status
        .success());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&wrapper).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0);
    }
    let ssot_agent = fs::read_to_string(dir.path().join(".agents/agents/executor.toml")).unwrap();
    let ssot_agent: toml::Value = toml::from_str(&ssot_agent).unwrap();
    assert!(ssot_agent.get("instructions").is_some());
    assert!(ssot_agent.get("developer_instructions").is_none());

    let codex_agent = fs::read_to_string(dir.path().join(".codex/agents/executor.toml")).unwrap();
    let codex_agent: toml::Value = toml::from_str(&codex_agent).unwrap();
    assert!(codex_agent
        .get("developer_instructions")
        .and_then(toml::Value::as_str)
        .is_some_and(|instructions| instructions.contains("# Executor")));
    assert!(codex_agent.get("instructions").is_none());
    toml::from_str::<toml::Value>(
        &fs::read_to_string(dir.path().join(".codex/config.toml")).unwrap(),
    )
    .unwrap();
    let hooks_json = fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap();
    let hooks: serde_json::Value = serde_json::from_str(&hooks_json).unwrap();
    assert!(hooks_json.contains("megara-caveman-SessionStart"));
    assert!(hooks_json.contains(r#""matcher": "startup|resume""#));
    assert!(hooks_json.contains("CAVEMAN MODE ACTIVE"));
    assert!(hooks_json.contains("megara-hook-UserPromptSubmit"));
    assert!(hooks_json.contains("megara-hook-PreToolUse"));
    assert!(
        hooks_json.contains("hook --managed-marker MEGARA:MANAGED --scope project --project-root")
    );
    assert!(hooks_json.contains("--runtime codex --event UserPromptSubmit"));
    let command = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(command.starts_with('"'));
    assert!(!command.starts_with("megara hook"));
    assert!(!hooks_json.contains("megara-hook.sh"));
    assert!(!hooks_json.contains("python3"));
    assert!(!hooks_json.contains(r#""matcher": "Bash""#));
    let megara_config = fs::read_to_string(dir.path().join(".agents/megara.toml")).unwrap();
    assert!(megara_config.contains("locale = \"ko-KR\""));
    assert!(megara_config.contains("default_active_skills = [\"caveman\"]"));
    let agents_md = fs::read_to_string(dir.path().join(".codex/AGENTS.md")).unwrap();
    assert!(agents_md.contains("## Locale"));
    assert!(agents_md.contains("Locale: `ko-KR`"));
    assert!(agents_md.contains("## Codex Runtime Adapter"));
    assert!(agents_md.contains("This projected harness is running inside Codex"));
    assert!(agents_md.contains("Codex native Plan mode is available through `/plan` or Shift+Tab"));
    assert!(agents_md.contains("Codex Plan-Mode Preflight before Round 0"));
    assert!(agents_md.contains("## Skills"));
    assert!(agents_md.contains("## Default Active Skills"));
    assert!(agents_md.contains("- `caveman`"));
    assert!(agents_md.contains("Do not mix languages in explanatory prose"));
    assert!(agents_md.contains("progress updates, clarification questions, option labels"));
    assert!(agents_md.contains("stock English workflow phrases"));
    assert!(agents_md.contains("Do not copy English workflow headings"));
    assert!(agents_md.contains("free-text values such as `question`, `options`"));
}
