/// Canonical task lifecycle phases. This is the source of truth — ship-types
/// re-exports or maps to these.
///
/// Note: `WaitingForHuman` is NOT a task phase. It's a session-level overlay
/// (`pending_human_review`) that can be set/cleared independently of the task
/// phase. When the human responds, the overlay clears and the task continues
/// from whatever phase it was already in.
///
/// Note: Steering is NOT a phase. The captain steers by @mentioning the mate
/// with feedback — this sends the task back to Working directly. There is no
/// intermediate "steer pending" state.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, facet::Facet)]
pub enum TaskPhase {
    Assigned,
    Working,
    PendingReview,
    RebaseConflict,
    Accepted,
    Cancelled,
}

impl TaskPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Cancelled)
    }
}

/// Is the transition from `from` to `to` valid?
///
/// Rules:
/// - Terminal states (Accepted, Cancelled) cannot be left.
/// - Any non-terminal state can transition to Cancelled.
/// - Everything else is an explicit allowlist.
pub fn can_transition(from: TaskPhase, to: TaskPhase) -> bool {
    if from.is_terminal() {
        return false;
    }

    if to == TaskPhase::Cancelled {
        return true;
    }

    matches!(
        (from, to),
        // Assignment phase — Assigned → Working is triggered by the mate's first
        // mutation op (the system transitions automatically, not an explicit action).
        (TaskPhase::Assigned, TaskPhase::Working)
            | (TaskPhase::Assigned, TaskPhase::Accepted)
            | (TaskPhase::Assigned, TaskPhase::RebaseConflict)
            // Work phase
            | (TaskPhase::Working, TaskPhase::PendingReview)
            // Review phase — captain can steer (sends mate back to Working),
            // accept, or hit a rebase conflict.
            | (TaskPhase::PendingReview, TaskPhase::Working)
            | (TaskPhase::PendingReview, TaskPhase::Accepted)
            | (TaskPhase::PendingReview, TaskPhase::RebaseConflict)
            // Rebase conflict resolution
            | (TaskPhase::RebaseConflict, TaskPhase::PendingReview)
            | (TaskPhase::RebaseConflict, TaskPhase::Accepted)
    )
}

/// All phases reachable from a given phase.
pub fn reachable_from(phase: TaskPhase) -> Vec<TaskPhase> {
    ALL_PHASES
        .iter()
        .copied()
        .filter(|&to| can_transition(phase, to))
        .collect()
}

const ALL_PHASES: &[TaskPhase] = &[
    TaskPhase::Assigned,
    TaskPhase::Working,
    TaskPhase::PendingReview,
    TaskPhase::RebaseConflict,
    TaskPhase::Accepted,
    TaskPhase::Cancelled,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_cannot_transition() {
        for &to in ALL_PHASES {
            assert!(!can_transition(TaskPhase::Accepted, to));
            assert!(!can_transition(TaskPhase::Cancelled, to));
        }
    }

    #[test]
    fn any_non_terminal_can_cancel() {
        let non_terminal = [
            TaskPhase::Assigned,
            TaskPhase::Working,
            TaskPhase::PendingReview,
            TaskPhase::RebaseConflict,
        ];
        for from in non_terminal {
            assert!(
                can_transition(from, TaskPhase::Cancelled),
                "{from:?} should be cancellable"
            );
        }
    }

    #[test]
    fn happy_path() {
        assert!(can_transition(TaskPhase::Assigned, TaskPhase::Working));
        assert!(can_transition(TaskPhase::Working, TaskPhase::PendingReview));
        assert!(can_transition(TaskPhase::PendingReview, TaskPhase::Accepted));
    }

    #[test]
    fn steer_sends_back_to_working() {
        // Captain steers by @mentioning mate → PendingReview goes back to Working.
        assert!(can_transition(TaskPhase::PendingReview, TaskPhase::Working));
        assert!(can_transition(TaskPhase::Working, TaskPhase::PendingReview));
    }

    #[test]
    fn rebase_conflict_flow() {
        assert!(can_transition(TaskPhase::PendingReview, TaskPhase::RebaseConflict));
        assert!(can_transition(TaskPhase::RebaseConflict, TaskPhase::PendingReview));
        assert!(can_transition(TaskPhase::RebaseConflict, TaskPhase::Accepted));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!can_transition(TaskPhase::Working, TaskPhase::Assigned));
        assert!(!can_transition(TaskPhase::Working, TaskPhase::Accepted));
        assert!(!can_transition(TaskPhase::Assigned, TaskPhase::PendingReview));
    }

    #[test]
    fn reachable_from_assigned() {
        let reachable = reachable_from(TaskPhase::Assigned);
        assert!(reachable.contains(&TaskPhase::Working));
        assert!(reachable.contains(&TaskPhase::Cancelled));
        assert!(reachable.contains(&TaskPhase::Accepted));
        assert!(reachable.contains(&TaskPhase::RebaseConflict));
        assert!(!reachable.contains(&TaskPhase::PendingReview));
    }

    #[test]
    fn reachable_from_terminal_is_empty() {
        assert!(reachable_from(TaskPhase::Accepted).is_empty());
        assert!(reachable_from(TaskPhase::Cancelled).is_empty());
    }
}
