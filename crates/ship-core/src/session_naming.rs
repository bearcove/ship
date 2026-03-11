use ship_types::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionGitNames {
    pub slug: String,
    pub branch_name: String,
    pub worktree_dir: String,
}

impl SessionGitNames {
    pub fn from_session_id(id: &SessionId) -> Self {
        let slug = session_slug(id);
        let branch_name = format!("ship-{slug}");
        let worktree_dir = format!("@{slug}");
        Self {
            slug,
            branch_name,
            worktree_dir,
        }
    }
}

/// Generate a 4-char lowercase alphanumeric slug for a session's branch and
/// worktree directory. Takes chars 10-13 of the ULID (random portion, not
/// timestamp), which are Crockford base32 digits and remain lowercase-safe.
fn session_slug(id: &SessionId) -> String {
    id.0.chars()
        .skip(10)
        .take(4)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
