use super::{brief, model::*, store::*, *};
use crate::cli::{ScopeArg, UltragoalCreateGoalsArgs};

pub(super) fn run(
    paths: &UltragoalPaths,
    scope: ScopeArg,
    session_id: &str,
    args: UltragoalCreateGoalsArgs,
) -> Result<()> {
    if paths.goals_file.exists() && !args.force {
        bail!(
            "ultragoal session already exists at {}; pass --force to recreate it",
            paths.goals_file.display()
        );
    }
    if paths.dir.exists() && args.force {
        fs::remove_dir_all(&paths.dir)
            .with_context(|| format!("failed to reset {}", paths.dir.display()))?;
    }

    let brief_source = brief::read_brief_source(paths, &args)?;
    let brief = brief_source.content;
    if brief.trim().is_empty() {
        bail!("ultragoal brief is empty");
    }

    let timestamp = timestamp();
    let goals = parse_goals(&brief)?
        .into_iter()
        .enumerate()
        .map(|(index, parsed)| UltragoalGoal {
            id: format!("G{:03}", index + 1),
            title: parsed.title,
            objective: parsed.objective,
            status: "pending".to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            started_at: None,
            completed_at: None,
            evidence: None,
            completion_receipt: None,
        })
        .collect::<Vec<_>>();

    fs::create_dir_all(&paths.evidence_dir)?;
    write_text_atomic(&paths.brief_file, ensure_trailing_newline(&brief).as_str())?;
    let plan = UltragoalPlan {
        version: 1,
        scope: scope_label(scope).to_string(),
        session_id: session_id.to_string(),
        brief_path: paths.brief_file.display().to_string(),
        brief_sha256: sha256_hex(brief.as_bytes()),
        source: Some(brief_source.source),
        goals,
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
    };
    write_plan(paths, &plan)?;
    write_runtime_state(paths, &plan, "goal_planning", true, &timestamp)?;
    mark_source_transition_started(paths, &plan, &timestamp)?;
    append_ledger(
        paths,
        &json!({
            "timestamp": timestamp,
            "event": "goals_created",
            "session_id": session_id,
            "source": plan.source.clone(),
            "goal_count": plan.goals.len(),
            "brief_path": paths.brief_file.display().to_string(),
            "goals_path": paths.goals_file.display().to_string(),
        }),
    )?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!(
            "ultragoal session {}: created {} goal(s)",
            plan.session_id,
            plan.goals.len()
        );
    }
    Ok(())
}
