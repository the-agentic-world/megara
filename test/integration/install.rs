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
    assert!(stdout.contains("Megara / Install"));
    assert!(stdout.contains("open a new session after install"));
    assert!(dir.path().join(".agents/megara.toml").exists());
    assert!(dir.path().join(".agents/.gitignore").exists());
    assert!(dir.path().join(".megara/.gitignore").exists());
    assert!(dir.path().join(".agents/bin/megara").exists());
    assert!(dir.path().join(".agents/bin/insane-search").exists());
    assert!(dir.path().join(".codex/AGENTS.md").exists());
    assert!(dir
        .path()
        .join(".agents/skills/deep-interview/SKILL.md")
        .exists());
    assert!(dir.path().join(".agents/skills/caveman/SKILL.md").exists());
    assert!(dir
        .path()
        .join(".agents/skills/insane-search/SKILL.md")
        .exists());
    assert!(!dir
        .path()
        .join(".codex/skills/deep-interview/SKILL.md")
        .exists());
    assert!(!dir.path().join(".codex/skills/caveman/SKILL.md").exists());
    assert!(dir
        .path()
        .join(".agents/tools/insane-search/TOOL.md")
        .exists());
    assert!(dir
        .path()
        .join(".agents/tools/insane-search/engine/__main__.py")
        .exists());
    assert!(dir
        .path()
        .join(".agents/tools/insane-search/requirements.txt")
        .exists());
    assert!(dir
        .path()
        .join(".agents/tools/insane-search/engine/templates/playwright_real_chrome.js")
        .exists());
    assert!(dir
        .path()
        .join(".agents/tools/insane-search/references/public-api.md")
        .exists());
    assert!(!dir
        .path()
        .join(".codex/skills/insane-search/SKILL.md")
        .exists());
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
    let agents_gitignore = fs::read_to_string(dir.path().join(".agents/.gitignore")).unwrap();
    assert!(agents_gitignore.contains("MEGARA:MANAGED"));
    assert!(agents_gitignore.contains("state/"));
    let runtime_gitignore = fs::read_to_string(dir.path().join(".megara/.gitignore")).unwrap();
    assert!(runtime_gitignore.contains("MEGARA:MANAGED"));
    assert!(runtime_gitignore.contains("state/"));
    assert!(runtime_gitignore.contains("artifacts/"));
    assert!(runtime_gitignore.contains("cache/"));
    let ultragoal =
        fs::read_to_string(dir.path().join(".agents/skills/ultragoal/SKILL.md")).unwrap();
    assert!(ultragoal.contains("Verification Evidence"));
    assert!(ultragoal.contains("Do not create, edit, copy, link, or list Megara runtime files"));
    assert!(ultragoal.contains("Quality gate JSON may be passed inline"));
    assert!(ultragoal.contains("artifactRefs` is optional"));
    assert!(!ultragoal.contains("Stable Evidence Directory"));
    assert!(!ultragoal.contains(".megara/artifacts/ultragoal/<session-id>/evidence/"));
    assert!(ultragoal.contains("ultragoal 승인"));
    assert!(ultragoal.contains("실행 단위 생성"));
    assert!(ultragoal.contains("Do not mention goals being opened, selected, approved, converted, or split into execution units"));
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
    assert!(skill.contains("show `0%` as the final visible ambiguity score"));
    assert!(skill.contains("Continue deep-interview to the next ambiguity target"));
    assert!(skill.contains("reaching the active target opens the milestone decision step"));
    assert!(skill.contains("do not crystallize at `1%`"));
    assert!(skill.contains("Runtime-Backed Multi-Turn Contract"));
    assert!(skill.contains("Codex App delegation wrappers"));
    assert!(skill.contains("Use subagents for lateral review"));
    assert!(skill.contains("must not call tools, read files, write files"));
    assert!(skill.contains("Use a minimal fact pass"));
    assert!(skill.contains("do not block the immediate next question"));
    assert!(skill.contains("Ask one compact follow-up from the confirmed topology"));
    assert!(skill.contains("read at most"));
    assert!(skill.contains("request exactly one subagent reviewer"));
    assert!(skill.contains("must forbid tool calls and file reads"));
    assert!(skill.contains("implementation mutation is blocked by the runtime until `ralplan`"));
    assert!(skill.contains("does not require Codex Plan mode"));
    assert!(skill.contains("Do not ask the user to toggle `/plan`"));
    assert!(!skill.contains("Codex Plan-Mode Activation"));
    assert!(!skill.contains("Runtime hooks attempt to activate Codex Plan mode before Round 0"));
    assert!(!skill.contains("activate Codex Plan mode first"));
    assert!(!skill.contains("A `/plan` text prefix is not enough by itself"));
    assert!(!skill.contains("Do not offer a \"continue without Plan mode\" option"));
    assert!(!skill.contains("Continue here without `/plan`"));
    assert!(!skill.contains("The preflight question must have"));
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
    assert!(skill.contains("output only the user-facing markdown spec"));
    assert!(skill.contains("Produce a user-friendly pending-approval summary"));
    assert!(skill.contains("For a `0%` target completion, this must be exactly `0%`"));
    assert!(skill.contains("Do not show raw labels such as `Metadata`"));
    assert!(!skill.contains("Interview ledger update:"));
    assert!(!skill.contains("Megara Question Gate:"));
    assert!(!skill.contains("Megara Workflow State:"));
    assert!(skill.contains("Do not emit `Megara Workflow State`"));
    assert!(skill.contains("locked markdown artifact"));
    assert!(skill.contains("spec_path"));
    assert!(skill.contains("pipeline_lock"));
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
    let insane_wrapper = fs::read_to_string(dir.path().join(".agents/bin/insane-search")).unwrap();
    assert!(insane_wrapper.contains(r#"runtime_root="$root_dir/../.megara""#));
    assert!(insane_wrapper.contains("state/tools/insane-search"));
    assert!(insane_wrapper.contains("python3 -m venv"));
    assert!(insane_wrapper.contains("pip install -r"));
    assert!(insane_wrapper.contains("requirements.stamp"));
    assert!(insane_wrapper.contains("-nt \"$requirements_stamp\""));
    assert!(insane_wrapper.contains("curl_cffi"));
    assert!(insane_wrapper.contains("yt_dlp"));
    assert!(insane_wrapper.contains(r#"exec "$python_bin" -m engine "$@""#));
    let insane_tool =
        fs::read_to_string(dir.path().join(".agents/tools/insane-search/TOOL.md")).unwrap();
    assert!(insane_tool.contains("kind: tool"));
    assert!(insane_tool.contains("not a default active skill"));
    assert!(insane_tool.contains("https://github.com/fivetaku/insane-search"));
    assert!(insane_tool.contains("bootstraps dependencies on first use"));
    assert!(insane_tool.contains(".megara/state/tools/insane-search/venv"));
    let insane_skill =
        fs::read_to_string(dir.path().join(".agents/skills/insane-search/SKILL.md")).unwrap();
    assert!(insane_skill.contains("name: insane-search"));
    assert!(insane_skill.contains("on-demand, not a default active skill"));
    assert!(insane_skill.contains(".agents/tools/insane-search/TOOL.md"));
    assert!(insane_skill.contains(".agents/bin/insane-search"));
    assert!(insane_skill.contains(".megara/state/tools/insane-search/venv"));
    let insane_requirements = fs::read_to_string(
        dir.path()
            .join(".agents/tools/insane-search/requirements.txt"),
    )
    .unwrap();
    assert!(insane_requirements.starts_with("# MEGARA:MANAGED"));
    assert!(insane_requirements.contains("curl_cffi>=0.15.0"));
    let insane_engine = fs::read_to_string(
        dir.path()
            .join(".agents/tools/insane-search/engine/__main__.py"),
    )
    .unwrap();
    assert!(insane_engine.starts_with("# MEGARA:MANAGED"));
    assert!(!insane_engine.contains("<!-- MEGARA:MANAGED"));
    let insane_yaml = fs::read_to_string(
        dir.path()
            .join(".agents/tools/insane-search/engine/waf_profiles.yaml"),
    )
    .unwrap();
    assert!(insane_yaml.starts_with("# MEGARA:MANAGED"));
    let insane_js = fs::read_to_string(
        dir.path()
            .join(".agents/tools/insane-search/engine/templates/playwright_real_chrome.js"),
    )
    .unwrap();
    assert!(insane_js.starts_with("// MEGARA:MANAGED"));
    serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(
            dir.path()
                .join(".agents/tools/insane-search/engine/templates/package.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let ralplan = fs::read_to_string(dir.path().join(".agents/skills/ralplan/SKILL.md")).unwrap();
    assert!(!ralplan.contains("Megara Review Pass:"));
    assert!(!ralplan.contains("Megara Plan Gate:"));
    assert!(!ralplan.contains("Megara Approval Gate:"));
    assert!(ralplan.contains("Do not output runtime metadata"));
    assert!(ralplan.contains("Runtime hooks record subagent receipts"));
    assert!(ralplan
        .contains("Do not send progress messages that merely narrate internal workflow mechanics"));
    assert!(ralplan.contains(
        "begin planning from the locked spec without asking another transition question"
    ));
    assert!(ralplan.contains("Normal user approval should be a number or natural-language choice"));
    assert!(ralplan.contains("hooks inject a hidden requirement"));
    assert!(ralplan.contains("input_spec_sha256"));
    assert!(ralplan.contains("plan_sha256"));
    assert!(ralplan.contains("pending-approval plan"));
    assert!(ralplan.contains("Baseline failure handling"));
    assert!(ralplan.contains("pre-existing"));
    assert!(ralplan.contains("Plan-owned clarification"));
    assert!(ralplan.contains("Do not block on details"));
    assert!(ralplan.contains("Pick the stricter product-facing"));
    assert!(ralplan.contains("generic list of unresolved review notes"));
    assert!(ralplan.contains("Do not put workflow or handoff names"));
    assert!(ralplan.contains("final numbered approval choices"));
    let ultragoal =
        fs::read_to_string(dir.path().join(".agents/skills/ultragoal/SKILL.md")).unwrap();
    assert!(ultragoal.contains(r#"MEGARA_BIN="${MEGARA_BIN:-.agents/bin/megara}""#));
    assert!(ultragoal.contains(r#""$MEGARA_BIN" ultragoal"#));
    assert!(ultragoal.contains("output only user-facing prose"));
    assert!(ultragoal.contains("Run Megara CLI commands silently"));
    assert!(ultragoal.contains("Do not narrate session ids"));
    assert!(ultragoal.contains("Runtime artifact paths under `.megara/state`"));
    assert!(ultragoal.contains("Do not link them, cite them as deliverables"));
    assert!(ultragoal.contains("runtime files are owned by Megara hooks and CLI commands"));
    assert!(ultragoal.contains("checkpoint attempts"));
    assert!(ultragoal.contains("start-goal"));
    assert!(ultragoal
        .contains("User-visible progress should mention only externally meaningful product work"));
    assert!(ultragoal.contains("Runtime state is managed by the `megara ultragoal` CLI commands"));
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
    assert!(hooks_json.contains("megara-hook-SubagentStart"));
    assert!(hooks_json.contains("megara-hook-SubagentStop"));
    assert!(
        hooks_json.contains("hook --managed-marker MEGARA:MANAGED --scope project --project-root")
    );
    assert!(hooks_json.contains("--runtime codex --event UserPromptSubmit"));
    assert!(hooks_json.contains("--runtime codex --event SubagentStart"));
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
    assert!(megara_config.contains("enabled_tools = [\"insane-search\"]"));
    let agents_md = fs::read_to_string(dir.path().join(".codex/AGENTS.md")).unwrap();
    assert!(agents_md.contains("## Locale"));
    assert!(agents_md.contains("Locale: `ko-KR`"));
    assert!(agents_md.contains("Hook output and Megara CLI state are runtime internals"));
    assert!(agents_md.contains("Runtime artifact paths under `.megara/state`"));
    assert!(agents_md.contains("Do not link them, cite them as deliverables"));
    assert!(agents_md.contains("quality-gate JSON"));
    assert!(agents_md.contains("block completion until agent-created changes are committed"));
    assert!(agents_md.contains("OMA `/scm`-style Conventional Commits"));
    assert!(agents_md.contains("never `git add .` or `git add -A`"));
    assert!(agents_md.contains("## Codex Runtime Adapter"));
    assert!(agents_md.contains("This projected harness is running inside Codex"));
    assert!(agents_md.contains("Deep-interview does not require Codex Plan mode"));
    assert!(agents_md.contains("Do not ask the user to toggle `/plan`"));
    assert!(!agents_md.contains("Megara hooks try to activate Codex Plan mode before Round 0"));
    assert!(!agents_md.contains("activate Plan mode, then resend"));
    assert!(!agents_md.contains("Do not offer a \"continue without Plan mode\" path"));
    assert!(agents_md.contains("delegated prompts may arrive wrapped"));
    assert!(agents_md.contains("implementation mutation is blocked until `ralplan`"));
    assert!(agents_md.contains("SubagentStart"));
    assert!(agents_md.contains("required receipts exist"));
    assert!(agents_md.contains("## Skills"));
    assert!(agents_md.contains("## On-Demand Tools"));
    assert!(agents_md.contains("insane-search"));
    assert!(agents_md.contains("- `insane-search`"));
    assert!(agents_md.contains("tools/insane-search/TOOL.md"));
    assert!(agents_md.contains("On-demand tools are not default active skills"));
    assert!(agents_md.contains(".agents/bin/<tool-name>"));
    assert!(agents_md.contains("## Default Active Skills"));
    assert!(agents_md.contains("- `caveman`"));
    assert!(agents_md.contains("Do not mix languages in explanatory prose"));
    assert!(agents_md.contains("progress updates, clarification questions, option labels"));
    assert!(agents_md.contains("stock English workflow phrases"));
    assert!(agents_md.contains("Do not copy English workflow headings"));
    assert!(agents_md.contains("free-text values such as `question`, `options`"));
}
