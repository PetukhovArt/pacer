use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Github,
    Gitlab,
    Jira,
    Confluence,
}

impl Kind {
    pub fn is_forge(self) -> bool {
        matches!(self, Kind::Github | Kind::Gitlab)
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Github => "GitHub",
            Kind::Gitlab => "GitLab",
            Kind::Jira => "Jira",
            Kind::Confluence => "Confluence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Cli,
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
    Unreachable,
    Unchecked,
}

#[derive(Debug, Clone, Serialize)]
pub struct Person {
    pub name: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

impl Person {
    pub fn ghost() -> Self {
        Person {
            name: "ghost".into(),
            username: "ghost".into(),
            avatar_url: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineState {
    Success,
    Failed,
    Running,
    Pending,
    Canceled,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CrState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Author,
    Reviewer,
    Assignee,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeRequestSummary {
    pub connector: String,
    pub project: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: CrState,
    pub draft: bool,
    pub author: Person,
    pub source_branch: String,
    pub target_branch: String,
    pub updated_at: String,
    pub pipeline: PipelineState,
    pub unresolved_threads: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchRequest {
    pub connector: String,
    pub project: String,
    pub number: u64,
    pub kind: Kind,
    pub state: CrState,
}

pub fn pick_branch_request(found: Vec<BranchRequest>) -> Option<BranchRequest> {
    let rank = |state: CrState| match state {
        CrState::Open => 0,
        CrState::Merged => 1,
        CrState::Closed => 2,
    };
    found
        .into_iter()
        .enumerate()
        .min_by_key(|(order, request)| (rank(request.state), *order))
        .map(|(_, request)| request)
}

#[derive(Debug, Clone, Serialize)]
pub struct InboxItem {
    #[serde(flatten)]
    pub summary: ChangeRequestSummary,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeRequest {
    #[serde(flatten)]
    pub summary: ChangeRequestSummary,
    pub description: String,
    pub reviewers: Vec<Person>,
    pub labels: Vec<String>,
    pub created_at: String,
    pub mergeable: Option<bool>,
    pub head_sha: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffFile {
    pub old_path: String,
    pub new_path: String,
    pub status: FileStatus,
    pub patch: String,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
    pub too_large: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub path: String,
    pub new_line: Option<u32>,
    pub old_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub id: String,
    pub author: Person,
    pub body: String,
    pub created_at: String,
    pub system: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Discussion {
    pub id: String,
    pub resolvable: bool,
    pub resolved: bool,
    pub position: Option<Position>,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub stage: String,
    pub status: String,
    pub url: String,
    pub duration_s: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pipeline {
    pub id: String,
    pub status: PipelineState,
    pub url: String,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Serialize)]
pub struct InboxError {
    pub connector: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Inbox {
    pub items: Vec<InboxItem>,
    pub errors: Vec<InboxError>,
}

pub fn count_patch(patch: &str) -> (u32, u32) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in patch.lines() {
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fork {
    pub owner: String,
    pub http_url: String,
    pub ssh_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRepo {
    pub branch: String,
    pub fork: Option<Fork>,
    pub gone: bool,
    pub can_push: Option<bool>,
}
