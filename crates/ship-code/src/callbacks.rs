use eyre::Result;

/// Callbacks that ship-server implements to handle operations
/// that require server-side coordination.
///
/// Ship-code is a library — it can't route messages, trigger agent
/// turns, or manage sessions. These callbacks bridge that gap.
#[allow(async_fn_in_trait)]
pub trait EngineCallbacks: Send + Sync {
    /// Send a message from the current agent to another participant.
    /// The engine has already validated the recipient.
    async fn send_message(&self, to: &str, text: &str) -> Result<()>;

    /// Submit work for review. Only valid for the mate role.
    /// Returns when the captain has seen the submission.
    async fn submit(&self, summary: &str) -> Result<()>;

    /// Notify that a real commit was created (shadow commits squashed).
    /// Ship-server may want to update session state, run hooks, etc.
    async fn on_commit(&self, hash: &str, message: &str) -> Result<()>;

    /// Check whether mutations are currently allowed.
    /// Returns Ok(()) if allowed, Err with a reason if not.
    /// Used to enforce the worktree lock (e.g. mate is active,
    /// captain can't edit).
    fn check_mutation_allowed(&self) -> Result<()>;

    /// The role of the agent calling the tool ("captain" or "mate").
    fn caller_role(&self) -> &str;
}

/// A no-op implementation for testing.
pub struct TestCallbacks {
    pub role: String,
    pub mutations_allowed: bool,
}

impl TestCallbacks {
    pub fn mate() -> Self {
        Self {
            role: "mate".to_owned(),
            mutations_allowed: true,
        }
    }

    pub fn captain() -> Self {
        Self {
            role: "captain".to_owned(),
            mutations_allowed: true,
        }
    }

    pub fn captain_locked() -> Self {
        Self {
            role: "captain".to_owned(),
            mutations_allowed: false,
        }
    }
}

impl EngineCallbacks for TestCallbacks {
    async fn send_message(&self, _to: &str, _text: &str) -> Result<()> {
        Ok(())
    }

    async fn submit(&self, _summary: &str) -> Result<()> {
        Ok(())
    }

    async fn on_commit(&self, _hash: &str, _message: &str) -> Result<()> {
        Ok(())
    }

    fn check_mutation_allowed(&self) -> Result<()> {
        if self.mutations_allowed {
            Ok(())
        } else {
            eyre::bail!(
                "The mate is currently working. Wait for submission before editing."
            )
        }
    }

    fn caller_role(&self) -> &str {
        &self.role
    }
}
