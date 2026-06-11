use crate::config::Paths;
use crate::domain::{Provider, WorkItem};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueWorktree {
    pub path: PathBuf,
    pub branch_name: String,
    pub commit_footer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCleanup {
    pub path: PathBuf,
    pub removed: bool,
}

pub fn prepare_issue_worktree(
    _paths: &Paths,
    repo_path: &Path,
    work_item: &WorkItem,
) -> Result<IssueWorktree> {
    ensure_git_repository(repo_path)?;

    let branch_name = issue_branch_name(work_item);
    let worktree_path = issue_worktree_path(repo_path, work_item, &branch_name)?;

    if worktree_path.exists() {
        let current_branch = git_output(&worktree_path, ["branch", "--show-current"])?;
        if current_branch == branch_name {
            return Ok(IssueWorktree {
                path: worktree_path,
                branch_name,
                commit_footer: issue_commit_footer(work_item),
            });
        }

        bail!(
            "existing worktree {} is on branch {}, expected {}",
            worktree_path.display(),
            current_branch,
            branch_name
        );
    }

    let parent = worktree_path
        .parent()
        .context("failed to resolve worktree parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    git_status(
        repo_path,
        [
            "worktree",
            "add",
            "-B",
            branch_name.as_str(),
            worktree_path
                .to_str()
                .context("worktree path is not valid UTF-8")?,
            "HEAD",
        ],
    )?;

    Ok(IssueWorktree {
        path: worktree_path,
        branch_name,
        commit_footer: issue_commit_footer(work_item),
    })
}

pub fn issue_branch_name(work_item: &WorkItem) -> String {
    let branch_type = issue_branch_type(work_item);
    let slug = slugify(&work_item.title);

    match work_item.provider {
        Provider::GitHub => format!("{}/{}-{}", branch_type, work_item.number, slug),
        Provider::GitLab => format!("{}-{}-{}", work_item.number, branch_type, slug),
    }
}

pub fn cleanup_issue_worktree(
    _paths: &Paths,
    repo_path: &Path,
    work_item: &WorkItem,
    force: bool,
) -> Result<WorktreeCleanup> {
    ensure_git_repository(repo_path)?;

    let branch_name = issue_branch_name(work_item);
    let worktree_path = issue_worktree_path(repo_path, work_item, &branch_name)?;
    ensure_managed_worktree_path(repo_path, &worktree_path)?;

    if !worktree_path.exists() {
        return Ok(WorktreeCleanup {
            path: worktree_path,
            removed: false,
        });
    }

    let status = git_output(&worktree_path, ["status", "--porcelain"])?;
    if !status.is_empty() && !force {
        bail!(
            "worktree {} has uncommitted changes; rerun with --force to remove it",
            worktree_path.display()
        );
    }

    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "remove"]);
    if force {
        command.arg("--force");
    }
    command.arg(&worktree_path);
    let output = command
        .output()
        .with_context(|| format!("failed to execute git in {}", repo_path.display()))?;
    if !output.status.success() {
        bail!(
            "git failed in {}: {}",
            repo_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(WorktreeCleanup {
        path: worktree_path,
        removed: true,
    })
}

fn issue_commit_footer(work_item: &WorkItem) -> String {
    match work_item.provider {
        Provider::GitHub => format!("Refs #{}", work_item.number),
        Provider::GitLab => format!("Ref #{}", work_item.number),
    }
}

fn issue_branch_type(work_item: &WorkItem) -> &'static str {
    if has_any_label(work_item, &["hotfix", "urgent", "critical"]) {
        return "hotfix";
    }

    if has_any_label(work_item, &["bug", "fix", "bugfix"]) {
        return "bugfix";
    }

    if has_any_label(work_item, &["release"]) {
        return "release";
    }

    "feature"
}

fn has_any_label(work_item: &WorkItem, candidates: &[&str]) -> bool {
    work_item.labels.iter().any(|label| {
        let normalized = label.to_ascii_lowercase();
        candidates.iter().any(|candidate| normalized == *candidate)
    })
}

fn issue_worktree_path(
    repo_path: &Path,
    work_item: &WorkItem,
    branch_name: &str,
) -> Result<PathBuf> {
    let parent = repo_path
        .parent()
        .with_context(|| format!("{} has no parent directory", repo_path.display()))?;

    Ok(parent
        .join(".sisyphus-worktrees")
        .join(work_item.provider.as_str())
        .join(sanitize_path_component(&work_item.owner_or_namespace))
        .join(sanitize_path_component(&work_item.repo))
        .join(branch_name))
}

fn ensure_managed_worktree_path(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let root = repo_path
        .parent()
        .with_context(|| format!("{} has no parent directory", repo_path.display()))?
        .join(".sisyphus-worktrees");
    if !worktree_path.starts_with(&root) {
        bail!(
            "refusing to clean unmanaged worktree path {}",
            worktree_path.display()
        );
    }

    Ok(())
}

fn ensure_git_repository(repo_path: &Path) -> Result<()> {
    let inside = git_output(repo_path, ["rev-parse", "--is-inside-work-tree"])
        .with_context(|| format!("{} is not a git worktree", repo_path.display()))?;
    if inside != "true" {
        bail!("{} is not a git worktree", repo_path.display());
    }

    Ok(())
}

fn git_output<const N: usize>(repo_path: &Path, args: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute git in {}", repo_path.display()))?;

    if !output.status.success() {
        bail!(
            "git failed in {}: {}",
            repo_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_status<const N: usize>(repo_path: &Path, args: [&str; N]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute git in {}", repo_path.display()))?;

    if !output.status.success() {
        bail!(
            "git failed in {}: {}",
            repo_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }

        if slug.len() >= 48 {
            break;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "issue".to_string()
    } else {
        slug.to_string()
    }
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work_item(provider: Provider, title: &str, labels: Vec<String>) -> WorkItem {
        let source_url = match provider {
            Provider::GitHub => "https://github.com/acme/widgets/issues/42",
            Provider::GitLab => "https://gitlab.com/acme/widgets/-/issues/42",
        };
        let instance_url = match provider {
            Provider::GitHub => "https://github.com",
            Provider::GitLab => "https://gitlab.com",
        };

        WorkItem {
            provider,
            source_url: source_url.to_string(),
            instance_url: instance_url.to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: title.to_string(),
            body: String::new(),
            labels,
            comments: Vec::new(),
        }
    }

    #[test]
    fn branch_name_uses_git_flow_feature_prefix_and_issue_slug() {
        assert_eq!(
            issue_branch_name(&work_item(
                Provider::GitHub,
                "Build import flow!",
                Vec::new()
            )),
            "feature/42-build-import-flow"
        );
    }

    #[test]
    fn branch_name_uses_git_flow_bugfix_prefix_for_bug_labels() {
        assert_eq!(
            issue_branch_name(&work_item(
                Provider::GitHub,
                "Fix import flow!",
                vec!["bug".to_string()]
            )),
            "bugfix/42-fix-import-flow"
        );
    }

    #[test]
    fn gitlab_branch_name_starts_with_issue_number_for_crosslinking() {
        assert_eq!(
            issue_branch_name(&work_item(
                Provider::GitLab,
                "Fix import flow!",
                vec!["bug".to_string()]
            )),
            "42-bugfix-fix-import-flow"
        );
    }

    #[test]
    fn branch_name_falls_back_when_title_has_no_slug() {
        assert_eq!(
            issue_branch_name(&work_item(Provider::GitHub, "!!!", Vec::new())),
            "feature/42-issue"
        );
    }

    #[test]
    fn worktree_path_lives_next_to_registered_repo() {
        let repo_path = PathBuf::from("/workspace/widgets");
        let path = issue_worktree_path(
            &repo_path,
            &work_item(Provider::GitHub, "Build import flow", Vec::new()),
            "feature/42-build-import-flow",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(
                "/workspace/.sisyphus-worktrees/github/acme/widgets/feature/42-build-import-flow"
            )
        );
    }

    #[test]
    fn commit_footer_uses_provider_reference_style() {
        assert_eq!(
            issue_commit_footer(&work_item(
                Provider::GitHub,
                "Build import flow",
                Vec::new()
            )),
            "Refs #42"
        );
        assert_eq!(
            issue_commit_footer(&work_item(
                Provider::GitLab,
                "Build import flow",
                Vec::new()
            )),
            "Ref #42"
        );
    }
}
