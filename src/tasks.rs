use crate::domain::WorkItem;
use crate::storage::QueueItem;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTask {
    pub queue_item_id: i64,
    pub issue_url: String,
    pub repo_path: PathBuf,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWorkspace {
    pub path: PathBuf,
    pub branch_name: Option<String>,
    pub commit_footer: Option<String>,
}

impl AgentWorkspace {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            branch_name: None,
            commit_footer: None,
        }
    }
}

pub fn build_codex_task(queue_item: &QueueItem, repo_path: &Path) -> Result<AgentTask> {
    build_codex_task_in_workspace(queue_item, &AgentWorkspace::new(repo_path))
}

pub fn build_codex_task_in_workspace(
    queue_item: &QueueItem,
    workspace: &AgentWorkspace,
) -> Result<AgentTask> {
    let work_item: WorkItem = serde_json::from_str(&queue_item.payload)
        .with_context(|| format!("failed to parse queue item {} payload", queue_item.id))?;
    let prompt = build_codex_prompt(queue_item.id, &work_item, workspace);

    Ok(AgentTask {
        queue_item_id: queue_item.id,
        issue_url: work_item.source_url,
        repo_path: workspace.path.clone(),
        prompt,
    })
}

fn build_codex_prompt(
    queue_item_id: i64,
    work_item: &WorkItem,
    workspace: &AgentWorkspace,
) -> String {
    let labels = if work_item.labels.is_empty() {
        "(none)".to_string()
    } else {
        work_item.labels.join(", ")
    };
    let body = if work_item.body.trim().is_empty() {
        "(empty issue body)"
    } else {
        work_item.body.trim()
    };
    let comments = format_issue_comments(work_item);
    let git_rules = format_git_rules(workspace);

    format!(
        r#"You are handling a Sisyphus-dispatched issue.

Sisyphus queue item: {queue_item_id}
Issue URL: {issue_url}
Provider: {provider}
Repository: {owner}/{repo}
Local workspace: {repo_path}
Issue title: {title}
Issue state: {state}
Issue labels: {labels}

Issue body:
{body}

Issue comments:
{comments}

Git tracking rules:
{git_rules}

Clarification gate:
First, inspect the task for blocking ambiguity. If the task is not actionable, do not implement.
Return only a JSON object with these fields when clarification is required:
- "type": exactly "clarification_request"
- "blocking": true
- "summary": one concrete sentence explaining what prevents implementation
- "questions": one or more objects, each with:
  - "id": a specific snake_case identifier
  - "question": a concrete implementation or verification question
  - "options": concrete answer choices when useful, otherwise []

Do not copy schema descriptions into the JSON values. Ask only questions that materially affect implementation or verification. Prefer concrete options when possible.
If the task is actionable, state that no blocking clarification is needed and continue.

Execution rules:
- Do not scan GitHub or GitLab to decide what work to pick up; this issue is the assigned task.
- Do not switch away from the Sisyphus-provided workspace when one is specified.
- Follow the repository's native instructions, including AGENTS.md, OMA, skills, or workflows when Codex normally sees them.
- Sisyphus does not own Codex session storage. Use Codex-native session behavior.
- Verify the change before final response whenever the repository provides tests or checks.
- In the final response, include the Sisyphus queue item id and the verification performed.
"#,
        queue_item_id = queue_item_id,
        issue_url = work_item.source_url,
        provider = work_item.provider.as_str(),
        owner = work_item.owner_or_namespace.as_str(),
        repo = work_item.repo.as_str(),
        repo_path = workspace.path.display(),
        title = work_item.title.as_str(),
        state = work_item.state.as_str(),
        labels = labels,
        body = body,
        comments = comments,
        git_rules = git_rules
    )
}

fn format_git_rules(workspace: &AgentWorkspace) -> String {
    match (&workspace.branch_name, &workspace.commit_footer) {
        (Some(branch_name), Some(commit_footer)) => format!(
            "- Work only in the isolated git worktree shown as Local workspace.\n\
             - Keep the branch name exactly: {branch_name}\n\
             - If you create commits for this issue, include this footer in each commit message: {commit_footer}\n\
             - Do not rename the branch, delete the worktree, or edit the source checkout outside this workspace."
        ),
        _ => "- Use the repository's existing git workflow for this manual dispatch.".to_string(),
    }
}

fn format_issue_comments(work_item: &WorkItem) -> String {
    if work_item.comments.is_empty() {
        return "(no issue comments)".to_string();
    }

    work_item
        .comments
        .iter()
        .map(|comment| {
            let created_at = comment.created_at.as_deref().unwrap_or("unknown time");
            let body = if comment.body.trim().is_empty() {
                "(empty comment)"
            } else {
                comment.body.trim()
            };
            format!(
                "Comment by {author} at {created_at}:\n{body}",
                author = comment.author
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IssueComment, Provider, WorkItem};

    #[test]
    fn builds_codex_task_with_clarification_gate() {
        let work_item = WorkItem {
            provider: Provider::GitHub,
            source_url: "https://github.com/acme/widgets/issues/42".to_string(),
            instance_url: "https://github.com".to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: "Build import flow".to_string(),
            body: "Need an import flow.".to_string(),
            labels: vec!["sisyphus".to_string()],
            comments: vec![IssueComment {
                author: "alice".to_string(),
                body: "Use the compact import flow.".to_string(),
                created_at: Some("2026-06-10T00:00:00Z".to_string()),
            }],
        };
        let queue_item = QueueItem {
            id: 7,
            provider: "github".to_string(),
            issue_url: work_item.source_url.clone(),
            state: "queued".to_string(),
            payload: serde_json::to_string(&work_item).unwrap(),
        };

        let task = build_codex_task(&queue_item, Path::new("/tmp/repo")).unwrap();

        assert_eq!(task.queue_item_id, 7);
        assert!(
            task.prompt
                .contains("Issue URL: https://github.com/acme/widgets/issues/42")
        );
        assert!(task.prompt.contains("exactly \"clarification_request\""));
        assert!(!task.prompt.contains("short reason the task is blocked"));
        assert!(!task.prompt.contains("implementation-relevant question"));
        assert!(!task.prompt.contains("concrete option A"));
        assert!(
            task.prompt
                .contains("Follow the repository's native instructions")
        );
        assert!(task.prompt.contains("Issue comments:"));
        assert!(task.prompt.contains("Use the compact import flow."));
    }

    #[test]
    fn builds_codex_task_with_isolated_workspace_git_rules() {
        let work_item = WorkItem {
            provider: Provider::GitLab,
            source_url: "https://gitlab.com/acme/widgets/-/issues/42".to_string(),
            instance_url: "https://gitlab.com".to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: "Build import flow".to_string(),
            body: "Need an import flow.".to_string(),
            labels: vec!["sisyphus".to_string()],
            comments: Vec::new(),
        };
        let queue_item = QueueItem {
            id: 7,
            provider: "gitlab".to_string(),
            issue_url: work_item.source_url.clone(),
            state: "queued".to_string(),
            payload: serde_json::to_string(&work_item).unwrap(),
        };

        let task = build_codex_task_in_workspace(
            &queue_item,
            &AgentWorkspace {
                path: PathBuf::from(
                    "/tmp/sisyphus/worktrees/gitlab/acme/widgets/42-feature-build-import-flow",
                ),
                branch_name: Some("42-feature-build-import-flow".to_string()),
                commit_footer: Some("Ref #42".to_string()),
            },
        )
        .unwrap();

        assert!(task.prompt.contains("isolated git worktree"));
        assert!(
            task.prompt
                .contains("Keep the branch name exactly: 42-feature-build-import-flow")
        );
        assert!(
            task.prompt
                .contains("include this footer in each commit message: Ref #42")
        );
    }
}
