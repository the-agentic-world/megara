use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    GitHub,
    GitLab,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
        }
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "github" => Ok(Self::GitHub),
            "gitlab" => Ok(Self::GitLab),
            other => Err(format!("unsupported provider: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRef {
    pub provider: Provider,
    pub instance_url: String,
    pub owner_or_namespace: String,
    pub repo: String,
    pub number: u64,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    pub provider: Provider,
    pub source_url: String,
    pub instance_url: String,
    pub owner_or_namespace: String,
    pub repo: String,
    pub number: u64,
    pub state: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    #[serde(default)]
    pub comments: Vec<IssueComment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueComment {
    pub author: String,
    pub body: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl WorkItem {
    pub fn from_issue_ref(issue_ref: IssueRef) -> Self {
        Self {
            provider: issue_ref.provider,
            source_url: issue_ref.source_url,
            instance_url: issue_ref.instance_url,
            owner_or_namespace: issue_ref.owner_or_namespace,
            repo: issue_ref.repo,
            number: issue_ref.number,
            state: "open".to_string(),
            title: String::new(),
            body: String::new(),
            labels: Vec::new(),
            comments: Vec::new(),
        }
    }

    pub fn issue_url(&self) -> String {
        match self.provider {
            Provider::GitHub => format!(
                "{}/{}/{}/issues/{}",
                self.instance_url, self.owner_or_namespace, self.repo, self.number
            ),
            Provider::GitLab => format!(
                "{}/{}/{}/-/issues/{}",
                self.instance_url, self.owner_or_namespace, self.repo, self.number
            ),
        }
    }
}
