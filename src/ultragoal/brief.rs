use super::{model::*, store::*, *};
use crate::cli::UltragoalCreateGoalsArgs;

pub(super) fn read_brief_source(
    paths: &UltragoalPaths,
    args: &UltragoalCreateGoalsArgs,
) -> Result<BriefSource> {
    let has_direct_source = args.brief.is_some() || args.brief_file.is_some() || args.from_stdin;
    if has_direct_source {
        let content = read_brief(
            args.brief.clone(),
            args.brief_file.as_deref(),
            args.from_stdin,
        )?;
        if args.allow_direct {
            return Ok(BriefSource {
                content,
                source: direct_source(),
            });
        }

        if let Ok(approved) = approved_ralplan_handoff(paths) {
            let expected = approved
                .source
                .ralplan_plan_sha256
                .as_deref()
                .unwrap_or_default();
            if sha256_hex(content.as_bytes()) == expected {
                return Ok(approved);
            }
        }

        bail!(
            "direct ultragoal brief is blocked; omit brief flags to consume an approved ralplan handoff, or pass --allow-direct for an explicit direct run"
        );
    }

    approved_ralplan_handoff(paths)
}

fn read_brief(
    brief: Option<String>,
    brief_file: Option<&Path>,
    from_stdin: bool,
) -> Result<String> {
    if let Some(brief) = brief {
        return Ok(brief);
    }
    if let Some(path) = brief_file {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read brief file: {}", path.display()));
    }
    if from_stdin {
        let mut value = String::new();
        io::stdin().read_to_string(&mut value)?;
        return Ok(value);
    }
    bail!("provide --brief, --brief-file, or --from-stdin")
}

fn approved_ralplan_handoff(paths: &UltragoalPaths) -> Result<BriefSource> {
    let state = fs::read_to_string(&paths.ralplan_state_file).with_context(|| {
        format!(
            "approved ralplan handoff is missing at {}; approve a ralplan plan first or pass --allow-direct",
            paths.ralplan_state_file.display()
        )
    })?;
    let state = serde_json::from_str::<Value>(&state).with_context(|| {
        format!(
            "failed to parse ralplan handoff state: {}",
            paths.ralplan_state_file.display()
        )
    })?;
    if state.get("skill").and_then(Value::as_str) != Some(RALPLAN)
        || state.get("approval_status").and_then(Value::as_str) != Some("approved")
        || state.get("approved_handoff_target").and_then(Value::as_str) != Some(WORKFLOW)
    {
        bail!(
            "approved ralplan handoff is not ready for ultragoal; approve the pending plan for ultragoal first"
        );
    }

    let plan_path = state
        .get("plan_path")
        .and_then(Value::as_str)
        .context("approved ralplan handoff is missing plan_path")?;
    let expected_sha256 = state
        .get("approved_plan_sha256")
        .or_else(|| state.get("plan_sha256"))
        .and_then(Value::as_str)
        .context("approved ralplan handoff is missing approved_plan_sha256")?;
    let content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read approved ralplan plan: {plan_path}"))?;
    let actual_sha256 = sha256_hex(content.as_bytes());
    if actual_sha256 != expected_sha256 {
        bail!(
            "approved ralplan plan hash mismatch: expected {expected_sha256}, got {actual_sha256}"
        );
    }

    Ok(BriefSource {
        content,
        source: ralplan_source(&state, plan_path, expected_sha256),
    })
}

fn ralplan_source(state: &Value, plan_path: &str, expected_sha256: &str) -> UltragoalSource {
    UltragoalSource {
        kind: "ralplan".to_string(),
        ralplan_plan_id: state
            .get("approved_plan_id")
            .or_else(|| state.get("plan_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        ralplan_plan_path: Some(plan_path.to_string()),
        ralplan_plan_sha256: Some(expected_sha256.to_string()),
        input_spec_path: state
            .get("input_spec_path")
            .and_then(Value::as_str)
            .map(str::to_string),
        input_spec_sha256: state
            .get("input_spec_sha256")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn direct_source() -> UltragoalSource {
    UltragoalSource {
        kind: "direct".to_string(),
        ralplan_plan_id: None,
        ralplan_plan_path: None,
        ralplan_plan_sha256: None,
        input_spec_path: None,
        input_spec_sha256: None,
    }
}
