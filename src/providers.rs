use crate::config::ProviderTargetConfig;
use crate::domain::{IssueComment, IssueRef, Provider, WorkItem};
use anyhow::{Context, Result, bail};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

const USER_AGENT_VALUE: &str = "sisyphus/0.1";

pub async fn poll_provider_targets(targets: &[ProviderTargetConfig]) -> Result<Vec<WorkItem>> {
    let client = reqwest::Client::new();
    let mut work_items = Vec::new();

    for target in targets {
        match &target.kind {
            Provider::GitHub => {
                work_items.extend(fetch_github_issues(&client, target).await?);
            }
            Provider::GitLab => {
                work_items.extend(fetch_gitlab_issues(&client, target).await?);
            }
        }
    }

    Ok(work_items)
}

pub async fn fetch_issue(
    issue_ref: &IssueRef,
    configured_target: Option<&ProviderTargetConfig>,
) -> Result<WorkItem> {
    let client = reqwest::Client::new();
    let target = issue_target(issue_ref, configured_target)?;

    match issue_ref.provider {
        Provider::GitHub => fetch_github_issue(&client, &target, issue_ref).await,
        Provider::GitLab => fetch_gitlab_issue(&client, &target, issue_ref).await,
    }
}

pub async fn post_issue_comment(
    target: &ProviderTargetConfig,
    work_item: &WorkItem,
    body: &str,
) -> Result<()> {
    if target.kind != work_item.provider {
        bail!("provider target does not match work item provider");
    }

    let client = reqwest::Client::new();
    match work_item.provider {
        Provider::GitHub => post_github_issue_comment(&client, target, work_item, body).await,
        Provider::GitLab => post_gitlab_issue_comment(&client, target, work_item, body).await,
    }
}

pub fn parse_issue_url(input: &str) -> Result<IssueRef> {
    let normalized = input.trim().trim_end_matches('/');
    let (scheme, rest) = normalized
        .split_once("://")
        .context("issue URL must include a scheme")?;
    let (host, path) = rest
        .split_once('/')
        .context("issue URL must include a path")?;
    let instance_url = format!("{scheme}://{host}");
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match host {
        "github.com" => parse_github_issue_url(&instance_url, normalized, &segments),
        "gitlab.com" => parse_gitlab_issue_url(&instance_url, normalized, &segments),
        _ => bail!("unsupported issue host: {host}"),
    }
}

fn parse_github_issue_url(
    instance_url: &str,
    source_url: &str,
    segments: &[&str],
) -> Result<IssueRef> {
    if segments.len() != 4 || segments[2] != "issues" {
        bail!("expected GitHub issue URL shape: /owner/repo/issues/number");
    }

    Ok(IssueRef {
        provider: Provider::GitHub,
        instance_url: instance_url.to_string(),
        owner_or_namespace: segments[0].to_string(),
        repo: segments[1].to_string(),
        number: parse_issue_number(segments[3])?,
        source_url: source_url.to_string(),
    })
}

fn parse_gitlab_issue_url(
    instance_url: &str,
    source_url: &str,
    segments: &[&str],
) -> Result<IssueRef> {
    let Some(marker_index) = segments.iter().position(|segment| *segment == "-") else {
        bail!("expected GitLab issue URL shape: /namespace/project/-/issues/iid");
    };

    if segments.len() != marker_index + 3 || segments[marker_index + 1] != "issues" {
        bail!("expected GitLab issue URL shape: /namespace/project/-/issues/iid");
    }

    if marker_index < 2 {
        bail!("GitLab issue URL must include namespace and project");
    }

    let project_path = &segments[..marker_index];
    let repo = project_path[project_path.len() - 1].to_string();
    let owner_or_namespace = project_path[..project_path.len() - 1].join("/");

    Ok(IssueRef {
        provider: Provider::GitLab,
        instance_url: instance_url.to_string(),
        owner_or_namespace,
        repo,
        number: parse_issue_number(segments[marker_index + 2])?,
        source_url: source_url.to_string(),
    })
}

fn parse_issue_number(raw: &str) -> Result<u64> {
    raw.parse::<u64>()
        .with_context(|| format!("invalid issue number: {raw}"))
}

async fn fetch_github_issues(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
) -> Result<Vec<WorkItem>> {
    let instance_url = target.resolved_instance_url();
    let url = format!(
        "{}/repos/{}/{}/issues?state=open&per_page=50",
        api_base_url(&instance_url, Provider::GitHub),
        target.owner_or_namespace,
        target.repo
    );

    let mut request = client.get(url).headers(github_headers()?);
    if let Some(token) = target.auth_token()? {
        request = request.bearer_auth(token);
    }

    let response = request.send().await.context("failed to poll GitHub")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub polling failed with HTTP {status}");
    }

    let issues = response
        .json::<Vec<GitHubIssue>>()
        .await
        .context("failed to parse GitHub issue response")?;
    let mut work_items = Vec::new();
    for issue in issues
        .into_iter()
        .filter(|issue| issue.pull_request.is_none())
    {
        let mut work_item = issue.into_work_item(target, &instance_url);
        work_item.comments = fetch_github_issue_comments(client, target, &work_item).await?;
        work_items.push(work_item);
    }

    Ok(work_items)
}

async fn fetch_github_issue(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    issue_ref: &IssueRef,
) -> Result<WorkItem> {
    let instance_url = target.resolved_instance_url();
    let url = format!(
        "{}/repos/{}/{}/issues/{}",
        api_base_url(&instance_url, Provider::GitHub),
        target.owner_or_namespace,
        target.repo,
        issue_ref.number
    );

    let mut request = client.get(url).headers(github_headers()?);
    if let Some(token) = target.auth_token()? {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .context("failed to fetch GitHub issue")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub issue fetch failed with HTTP {status}");
    }

    let issue = response
        .json::<GitHubIssue>()
        .await
        .context("failed to parse GitHub issue response")?;
    if issue.pull_request.is_some() {
        bail!("GitHub URL points to a pull request, not an issue");
    }

    let mut work_item = issue.into_work_item(target, &instance_url);
    work_item.comments = fetch_github_issue_comments(client, target, &work_item).await?;
    Ok(work_item)
}

async fn fetch_gitlab_issues(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
) -> Result<Vec<WorkItem>> {
    let instance_url = target.resolved_instance_url();
    let project_path = format!("{}/{}", target.owner_or_namespace, target.repo);
    let encoded_project_path = urlencoding::encode(&project_path);
    let url = format!(
        "{}/api/v4/projects/{}/issues?state=opened&per_page=50",
        instance_url.trim_end_matches('/'),
        encoded_project_path
    );

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    let mut request = client.get(url).headers(headers);
    if let Some(token) = target.auth_token()? {
        request = request.header("PRIVATE-TOKEN", token);
    }

    let response = request.send().await.context("failed to poll GitLab")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitLab polling failed with HTTP {status}");
    }

    let issues = response
        .json::<Vec<GitLabIssue>>()
        .await
        .context("failed to parse GitLab issue response")?;
    let mut work_items = Vec::new();
    for issue in issues {
        let mut work_item = issue.into_work_item(target, &instance_url);
        work_item.comments = fetch_gitlab_issue_comments(client, target, &work_item).await?;
        work_items.push(work_item);
    }

    Ok(work_items)
}

async fn fetch_gitlab_issue(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    issue_ref: &IssueRef,
) -> Result<WorkItem> {
    let instance_url = target.resolved_instance_url();
    let project_path = format!("{}/{}", target.owner_or_namespace, target.repo);
    let encoded_project_path = urlencoding::encode(&project_path);
    let url = format!(
        "{}/api/v4/projects/{}/issues/{}",
        instance_url.trim_end_matches('/'),
        encoded_project_path,
        issue_ref.number
    );

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    let mut request = client.get(url).headers(headers);
    if let Some(token) = target.auth_token()? {
        request = request.header("PRIVATE-TOKEN", token);
    }

    let response = request
        .send()
        .await
        .context("failed to fetch GitLab issue")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitLab issue fetch failed with HTTP {status}");
    }

    let issue = response
        .json::<GitLabIssue>()
        .await
        .context("failed to parse GitLab issue response")?;
    let mut work_item = issue.into_work_item(target, &instance_url);
    work_item.comments = fetch_gitlab_issue_comments(client, target, &work_item).await?;
    Ok(work_item)
}

async fn fetch_github_issue_comments(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    work_item: &WorkItem,
) -> Result<Vec<IssueComment>> {
    let url = issue_comment_endpoint(target, work_item)?;
    let mut request = client.get(url).headers(github_headers()?);
    if let Some(token) = target.auth_token()? {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .context("failed to poll GitHub issue comments")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub issue comment polling failed with HTTP {status}");
    }

    let comments = response
        .json::<Vec<GitHubComment>>()
        .await
        .context("failed to parse GitHub issue comments")?;
    Ok(comments
        .into_iter()
        .map(GitHubComment::into_issue_comment)
        .collect())
}

async fn fetch_gitlab_issue_comments(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    work_item: &WorkItem,
) -> Result<Vec<IssueComment>> {
    let url = issue_comment_endpoint(target, work_item)?;
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    let mut request = client.get(url).headers(headers);
    if let Some(token) = target.auth_token()? {
        request = request.header("PRIVATE-TOKEN", token);
    }

    let response = request
        .send()
        .await
        .context("failed to poll GitLab issue comments")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitLab issue comment polling failed with HTTP {status}");
    }

    let comments = response
        .json::<Vec<GitLabNote>>()
        .await
        .context("failed to parse GitLab issue comments")?;
    Ok(comments
        .into_iter()
        .filter(|note| !note.system)
        .map(GitLabNote::into_issue_comment)
        .collect())
}

async fn post_github_issue_comment(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    work_item: &WorkItem,
    body: &str,
) -> Result<()> {
    let url = issue_comment_endpoint(target, work_item)?;
    let mut request = client
        .post(url)
        .headers(github_headers()?)
        .json(&CreateIssueComment { body });
    if let Some(token) = target.auth_token()? {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .context("failed to write GitHub issue comment")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub issue comment failed with HTTP {status}");
    }

    Ok(())
}

async fn post_gitlab_issue_comment(
    client: &reqwest::Client,
    target: &ProviderTargetConfig,
    work_item: &WorkItem,
    body: &str,
) -> Result<()> {
    let url = issue_comment_endpoint(target, work_item)?;
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    let mut request = client
        .post(url)
        .headers(headers)
        .json(&CreateIssueComment { body });
    if let Some(token) = target.auth_token()? {
        request = request.header("PRIVATE-TOKEN", token);
    }

    let response = request
        .send()
        .await
        .context("failed to write GitLab issue comment")?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitLab issue comment failed with HTTP {status}");
    }

    Ok(())
}

fn issue_comment_endpoint(target: &ProviderTargetConfig, work_item: &WorkItem) -> Result<String> {
    if target.kind != work_item.provider {
        bail!("provider target does not match work item provider");
    }

    let instance_url = target.resolved_instance_url();
    match work_item.provider {
        Provider::GitHub => Ok(format!(
            "{}/repos/{}/{}/issues/{}/comments",
            api_base_url(&instance_url, Provider::GitHub),
            target.owner_or_namespace,
            target.repo,
            work_item.number
        )),
        Provider::GitLab => {
            let project_path = format!("{}/{}", target.owner_or_namespace, target.repo);
            let encoded_project_path = urlencoding::encode(&project_path);
            Ok(format!(
                "{}/api/v4/projects/{}/issues/{}/notes",
                instance_url.trim_end_matches('/'),
                encoded_project_path,
                work_item.number
            ))
        }
    }
}

fn github_headers() -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );
    Ok(headers)
}

fn api_base_url(instance_url: &str, provider: Provider) -> String {
    match provider {
        Provider::GitHub if instance_url.trim_end_matches('/') == "https://github.com" => {
            "https://api.github.com".to_string()
        }
        _ => instance_url.trim_end_matches('/').to_string(),
    }
}

fn issue_target(
    issue_ref: &IssueRef,
    configured_target: Option<&ProviderTargetConfig>,
) -> Result<ProviderTargetConfig> {
    if let Some(target) = configured_target {
        if target.kind != issue_ref.provider
            || target.owner_or_namespace != issue_ref.owner_or_namespace
            || target.repo != issue_ref.repo
            || target.resolved_instance_url().trim_end_matches('/')
                != issue_ref.instance_url.trim_end_matches('/')
        {
            bail!("configured provider target does not match issue URL");
        }
        return Ok(target.clone());
    }

    Ok(ProviderTargetConfig {
        kind: issue_ref.provider.clone(),
        owner_or_namespace: issue_ref.owner_or_namespace.clone(),
        repo: issue_ref.repo.clone(),
        instance_url: Some(issue_ref.instance_url.clone()),
        token_env: None,
    })
}

#[derive(Debug, Serialize)]
struct CreateIssueComment<'a> {
    body: &'a str,
}

#[derive(Debug, Deserialize)]
struct GitHubIssue {
    html_url: String,
    number: u64,
    state: String,
    title: String,
    body: Option<String>,
    labels: Vec<GitHubLabel>,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

impl GitHubIssue {
    fn into_work_item(self, target: &ProviderTargetConfig, instance_url: &str) -> WorkItem {
        WorkItem {
            provider: Provider::GitHub,
            source_url: self.html_url,
            instance_url: instance_url.to_string(),
            owner_or_namespace: target.owner_or_namespace.clone(),
            repo: target.repo.clone(),
            number: self.number,
            state: self.state,
            title: self.title,
            body: self.body.unwrap_or_default(),
            labels: self.labels.into_iter().map(|label| label.name).collect(),
            comments: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubComment {
    body: Option<String>,
    created_at: Option<String>,
    user: GitHubUser,
}

impl GitHubComment {
    fn into_issue_comment(self) -> IssueComment {
        IssueComment {
            author: self.user.login,
            body: self.body.unwrap_or_default(),
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitLabIssue {
    web_url: String,
    iid: u64,
    state: String,
    title: String,
    description: Option<String>,
    labels: Vec<String>,
}

impl GitLabIssue {
    fn into_work_item(self, target: &ProviderTargetConfig, instance_url: &str) -> WorkItem {
        WorkItem {
            provider: Provider::GitLab,
            source_url: self.web_url,
            instance_url: instance_url.to_string(),
            owner_or_namespace: target.owner_or_namespace.clone(),
            repo: target.repo.clone(),
            number: self.iid,
            state: normalize_gitlab_state(&self.state),
            title: self.title,
            body: self.description.unwrap_or_default(),
            labels: self.labels,
            comments: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitLabNote {
    body: String,
    created_at: Option<String>,
    author: GitLabAuthor,
    #[serde(default)]
    system: bool,
}

impl GitLabNote {
    fn into_issue_comment(self) -> IssueComment {
        IssueComment {
            author: self.author.username,
            body: self.body,
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitLabAuthor {
    username: String,
}

fn normalize_gitlab_state(state: &str) -> String {
    match state {
        "opened" => "open".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn parses_github_issue_url() {
        let parsed = parse_issue_url("https://github.com/acme/widgets/issues/42").unwrap();
        assert_eq!(parsed.provider, Provider::GitHub);
        assert_eq!(parsed.instance_url, "https://github.com");
        assert_eq!(parsed.owner_or_namespace, "acme");
        assert_eq!(parsed.repo, "widgets");
        assert_eq!(parsed.number, 42);
    }

    #[test]
    fn parses_gitlab_issue_url_with_nested_namespace() {
        let parsed =
            parse_issue_url("https://gitlab.com/acme/platform/widgets/-/issues/7").unwrap();
        assert_eq!(parsed.provider, Provider::GitLab);
        assert_eq!(parsed.instance_url, "https://gitlab.com");
        assert_eq!(parsed.owner_or_namespace, "acme/platform");
        assert_eq!(parsed.repo, "widgets");
        assert_eq!(parsed.number, 7);
    }

    #[test]
    fn rejects_unsupported_hosts() {
        assert!(parse_issue_url("https://example.com/acme/widgets/issues/42").is_err());
    }

    #[test]
    fn maps_github_issue_response_to_work_item() {
        let target = ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: None,
        };
        let issue = GitHubIssue {
            html_url: "https://github.com/acme/widgets/issues/42".to_string(),
            number: 42,
            state: "open".to_string(),
            title: "Build the thing".to_string(),
            body: Some("body".to_string()),
            labels: vec![GitHubLabel {
                name: "sisyphus".to_string(),
            }],
            pull_request: None,
        };

        let item = issue.into_work_item(&target, "https://github.com");

        assert_eq!(item.provider, Provider::GitHub);
        assert_eq!(item.source_url, "https://github.com/acme/widgets/issues/42");
        assert_eq!(item.labels, vec!["sisyphus"]);
    }

    #[test]
    fn maps_gitlab_opened_state_to_open() {
        let target = ProviderTargetConfig {
            kind: Provider::GitLab,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: None,
        };
        let issue = GitLabIssue {
            web_url: "https://gitlab.com/acme/platform/widgets/-/issues/7".to_string(),
            iid: 7,
            state: "opened".to_string(),
            title: "Build the thing".to_string(),
            description: None,
            labels: vec!["sisyphus".to_string()],
        };

        let item = issue.into_work_item(&target, "https://gitlab.com");

        assert_eq!(item.provider, Provider::GitLab);
        assert_eq!(item.state, "open");
        assert_eq!(item.labels, vec!["sisyphus"]);
    }

    #[test]
    fn github_com_uses_public_api_base() {
        assert_eq!(
            api_base_url("https://github.com", Provider::GitHub),
            "https://api.github.com"
        );
        assert_eq!(
            api_base_url("https://github.example.com", Provider::GitHub),
            "https://github.example.com"
        );
    }

    #[test]
    fn builds_github_comment_endpoint() {
        let target = ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: None,
        };
        let work_item = WorkItem {
            provider: Provider::GitHub,
            source_url: "https://github.com/acme/widgets/issues/42".to_string(),
            instance_url: "https://github.com".to_string(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec![],
            comments: vec![],
        };

        assert_eq!(
            issue_comment_endpoint(&target, &work_item).unwrap(),
            "https://api.github.com/repos/acme/widgets/issues/42/comments"
        );
    }

    #[test]
    fn builds_gitlab_comment_endpoint_with_encoded_project_path() {
        let target = ProviderTargetConfig {
            kind: Provider::GitLab,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            instance_url: None,
            token_env: None,
        };
        let work_item = WorkItem {
            provider: Provider::GitLab,
            source_url: "https://gitlab.com/acme/platform/widgets/-/issues/7".to_string(),
            instance_url: "https://gitlab.com".to_string(),
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            number: 7,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec![],
            comments: vec![],
        };

        assert_eq!(
            issue_comment_endpoint(&target, &work_item).unwrap(),
            "https://gitlab.com/api/v4/projects/acme%2Fplatform%2Fwidgets/issues/7/notes"
        );
    }

    #[tokio::test]
    async fn polls_github_issues_from_http_endpoint() {
        let instance_url = mock_json_responses(vec![
            r#"[
              {
                "html_url": "https://github.example.com/acme/widgets/issues/42",
                "number": 42,
                "state": "open",
                "title": "Build polling",
                "body": "body",
                "labels": [{"name": "sisyphus"}]
              },
              {
                "html_url": "https://github.example.com/acme/widgets/pull/43",
                "number": 43,
                "state": "open",
                "title": "Skip PR",
                "body": null,
                "labels": [{"name": "sisyphus"}],
                "pull_request": {}
              }
            ]"#,
            r#"[
              {
                "body": "clarification answer",
                "created_at": "2026-06-10T00:00:00Z",
                "user": {"login": "alice"}
              }
            ]"#,
        ])
        .await;
        let target = ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: Some(instance_url.clone()),
            token_env: None,
        };

        let items = poll_provider_targets(&[target]).await.unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].provider, Provider::GitHub);
        assert_eq!(items[0].instance_url, instance_url);
        assert_eq!(items[0].number, 42);
        assert_eq!(items[0].title, "Build polling");
        assert_eq!(items[0].labels, vec!["sisyphus"]);
        assert_eq!(items[0].comments.len(), 1);
        assert_eq!(items[0].comments[0].author, "alice");
        assert_eq!(items[0].comments[0].body, "clarification answer");
    }

    #[tokio::test]
    async fn fetches_single_github_issue_from_http_endpoint() {
        let instance_url = mock_json_responses(vec![
            r#"{
              "html_url": "https://github.example.com/acme/widgets/issues/42",
              "number": 42,
              "state": "open",
              "title": "Build import fetch",
              "body": "body",
              "labels": [{"name": "sisyphus"}]
            }"#,
            r#"[
              {
                "body": "clarification answer",
                "created_at": "2026-06-10T00:00:00Z",
                "user": {"login": "alice"}
              }
            ]"#,
        ])
        .await;
        let issue_ref = IssueRef {
            provider: Provider::GitHub,
            instance_url: instance_url.clone(),
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            source_url: "https://github.example.com/acme/widgets/issues/42".to_string(),
        };

        let item = fetch_issue(&issue_ref, None).await.unwrap();

        assert_eq!(item.title, "Build import fetch");
        assert_eq!(item.body, "body");
        assert_eq!(item.labels, vec!["sisyphus"]);
        assert_eq!(item.comments.len(), 1);
    }

    #[tokio::test]
    async fn polls_gitlab_issues_from_http_endpoint() {
        let instance_url = mock_json_responses(vec![
            r#"[
              {
                "web_url": "https://gitlab.example.com/acme/platform/widgets/-/issues/7",
                "iid": 7,
                "state": "opened",
                "title": "Build polling",
                "description": "body",
                "labels": ["sisyphus"]
              }
            ]"#,
            r#"[
              {
                "body": "clarification answer",
                "created_at": "2026-06-10T00:00:00Z",
                "author": {"username": "alice"},
                "system": false
              },
              {
                "body": "system note",
                "author": {"username": "gitlab"},
                "system": true
              }
            ]"#,
        ])
        .await;
        let target = ProviderTargetConfig {
            kind: Provider::GitLab,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            instance_url: Some(instance_url.clone()),
            token_env: None,
        };

        let items = poll_provider_targets(&[target]).await.unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].provider, Provider::GitLab);
        assert_eq!(items[0].instance_url, instance_url);
        assert_eq!(items[0].number, 7);
        assert_eq!(items[0].state, "open");
        assert_eq!(items[0].labels, vec!["sisyphus"]);
        assert_eq!(items[0].comments.len(), 1);
        assert_eq!(items[0].comments[0].author, "alice");
        assert_eq!(items[0].comments[0].body, "clarification answer");
    }

    #[tokio::test]
    async fn fetches_single_gitlab_issue_from_http_endpoint() {
        let instance_url = mock_json_responses(vec![
            r#"{
              "web_url": "https://gitlab.example.com/acme/platform/widgets/-/issues/7",
              "iid": 7,
              "state": "opened",
              "title": "Build import fetch",
              "description": "body",
              "labels": ["sisyphus"]
            }"#,
            r#"[
              {
                "body": "clarification answer",
                "created_at": "2026-06-10T00:00:00Z",
                "author": {"username": "alice"},
                "system": false
              }
            ]"#,
        ])
        .await;
        let issue_ref = IssueRef {
            provider: Provider::GitLab,
            instance_url: instance_url.clone(),
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            number: 7,
            source_url: "https://gitlab.example.com/acme/platform/widgets/-/issues/7".to_string(),
        };

        let item = fetch_issue(&issue_ref, None).await.unwrap();

        assert_eq!(item.state, "open");
        assert_eq!(item.title, "Build import fetch");
        assert_eq!(item.body, "body");
        assert_eq!(item.labels, vec!["sisyphus"]);
        assert_eq!(item.comments.len(), 1);
    }

    #[tokio::test]
    async fn posts_github_issue_comment_to_http_endpoint() {
        let instance_url = mock_status_response("201 Created").await;
        let target = ProviderTargetConfig {
            kind: Provider::GitHub,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            instance_url: Some(instance_url.clone()),
            token_env: None,
        };
        let work_item = WorkItem {
            provider: Provider::GitHub,
            source_url: format!("{instance_url}/acme/widgets/issues/42"),
            instance_url,
            owner_or_namespace: "acme".to_string(),
            repo: "widgets".to_string(),
            number: 42,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec![],
            comments: vec![],
        };

        post_issue_comment(&target, &work_item, "clarify please")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn posts_gitlab_issue_comment_to_http_endpoint() {
        let instance_url = mock_status_response("201 Created").await;
        let target = ProviderTargetConfig {
            kind: Provider::GitLab,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            instance_url: Some(instance_url.clone()),
            token_env: None,
        };
        let work_item = WorkItem {
            provider: Provider::GitLab,
            source_url: format!("{instance_url}/acme/platform/widgets/-/issues/7"),
            instance_url,
            owner_or_namespace: "acme/platform".to_string(),
            repo: "widgets".to_string(),
            number: 7,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: vec![],
            comments: vec![],
        };

        post_issue_comment(&target, &work_item, "clarify please")
            .await
            .unwrap();
    }

    async fn mock_json_responses(bodies: Vec<&'static str>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            for body in bodies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 2048];
                let _ = stream.read(&mut buf).await.unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });

        format!("http://{addr}")
    }

    async fn mock_status_response(status: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0_u8; 2048];
            let read = stream.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..read]);
            assert!(request.starts_with("POST "));
            assert!(request.contains("clarify please"));
            let response =
                format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{addr}")
    }
}
