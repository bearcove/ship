/// Information about the GitHub repository associated with the current directory.
#[derive(Debug, Clone, facet::Facet)]
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub url: String,
}

/// A created or existing pull request.
#[derive(Debug, Clone, facet::Facet)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: PrState,
    pub head_branch: String,
    pub base_branch: String,
    pub draft: bool,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, facet::Facet)]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

/// Options for creating a pull request.
pub struct CreatePrOptions<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub draft: bool,
}

/// A single check/status on a PR or commit.
#[derive(Debug, Clone, facet::Facet)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}
