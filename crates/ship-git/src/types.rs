use std::fmt;

use camino::Utf8PathBuf;

macro_rules! newtype_string {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $name(String);

            impl $name {
                pub fn new(s: impl Into<String>) -> Self {
                    Self(s.into())
                }

                pub fn as_str(&self) -> &str {
                    &self.0
                }

                pub fn into_string(self) -> String {
                    self.0
                }
            }

            impl AsRef<str> for $name {
                fn as_ref(&self) -> &str {
                    &self.0
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str(&self.0)
                }
            }

            impl From<String> for $name {
                fn from(s: String) -> Self {
                    Self(s)
                }
            }

            impl From<&str> for $name {
                fn from(s: &str) -> Self {
                    Self(s.to_owned())
                }
            }
        )+
    };
}

newtype_string! {
    /// A full or abbreviated commit SHA.
    CommitHash,

    /// A branch name (e.g. "main", "feature/foo").
    BranchName,

    /// A revision specifier — could be a hash, branch name, tag, HEAD~3, etc.
    /// This is the input type for most git operations that accept a ref.
    Rev,

    /// Raw unified diff output.
    Diff,

    /// A remote name (e.g. "origin").
    RemoteName,
}

// CommitHash and BranchName can be used anywhere a Rev is expected.
impl From<CommitHash> for Rev {
    fn from(h: CommitHash) -> Self {
        Rev(h.0)
    }
}

impl From<&CommitHash> for Rev {
    fn from(h: &CommitHash) -> Self {
        Rev(h.0.clone())
    }
}

impl From<BranchName> for Rev {
    fn from(b: BranchName) -> Self {
        Rev(b.0)
    }
}

impl From<&BranchName> for Rev {
    fn from(b: &BranchName) -> Self {
        Rev(b.0.clone())
    }
}

/// Information about a newly created commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: CommitHash,
    pub subject: String,
}

/// A single entry from `git log --format="%h %s"`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub hash: CommitHash,
    pub subject: String,
}

/// One line from `git diff --numstat`.
#[derive(Debug, Clone)]
pub struct NumstatEntry {
    pub added: usize,
    pub removed: usize,
    pub path: Utf8PathBuf,
}

/// Aggregated numstat diff statistics.
#[derive(Debug, Clone)]
pub struct DiffStats {
    pub entries: Vec<NumstatEntry>,
}

impl DiffStats {
    pub fn total_added(&self) -> usize {
        self.entries.iter().map(|e| e.added).sum()
    }

    pub fn total_removed(&self) -> usize {
        self.entries.iter().map(|e| e.removed).sum()
    }

    pub fn files_changed(&self) -> usize {
        self.entries.len()
    }
}

/// Outcome of a rebase operation.
#[derive(Debug, Clone)]
pub enum RebaseOutcome {
    Success,
    Conflict {
        conflicting_files: Vec<Utf8PathBuf>,
    },
}

/// A single entry from `git status --porcelain`.
#[derive(Debug, Clone)]
pub struct StatusEntry {
    /// Index status character (X in XY).
    pub index: char,
    /// Worktree status character (Y in XY).
    pub worktree: char,
    /// File path.
    pub path: Utf8PathBuf,
}

/// Parsed output of `git status --porcelain`.
#[derive(Debug, Clone)]
pub struct Status {
    pub entries: Vec<StatusEntry>,
}

impl Status {
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has_staged(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.index != ' ' && e.index != '?')
    }

    pub fn has_unstaged(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.worktree != ' ' && e.worktree != '?')
    }

    pub fn has_untracked(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.index == '?' && e.worktree == '?')
    }
}
