/// Centralized message templates for captain↔mate communication.
///
/// Organized by audience: captain-facing (about the mate), mate-facing
/// (from the captain/system to the mate), and utility wrappers.

// ---------------------------------------------------------------------------
// Captain-facing messages (injected into the captain's feed)
// ---------------------------------------------------------------------------

pub fn mate_plan_set(plan_status: &str) -> String {
    format!(
        "@captain I've set my plan.\n\n\
         {plan_status}\n\n\
         I'll keep you posted as I progress."
    )
}

pub fn mate_step_committed(
    step_description: &str,
    commit_summary: &str,
    diff_section: &str,
) -> String {
    format!("@captain Completed step: {step_description}\n\n{commit_summary}{diff_section}")
}

pub fn mate_committed_no_step(commit_summary: &str, diff_section: &str) -> String {
    format!("@captain Committed:\n\n{commit_summary}{diff_section}")
}

pub fn mate_update(message: &str) -> String {
    format!("@captain {message}")
}

pub fn mate_submitted(summary: &str) -> String {
    format!(
        "@captain I've submitted my work for review: {summary}\n\n_(to reply, @mate your message)_"
    )
}

pub fn mate_question(question: &str) -> String {
    format!("@captain {question}\n\n_(to reply, @mate your message)_")
}

pub fn mate_activity_summary(summary: &str) -> String {
    format!("@captain [Summarizer]\n{summary}")
}

pub fn captain_unaddressed_bounce() -> String {
    "Your last message didn't address anyone. \
     Please start messages with @mate, @human, or @admiral."
        .to_owned()
}

// ---------------------------------------------------------------------------
// Mate-facing messages (injected into the mate's feed)
// ---------------------------------------------------------------------------

pub fn captain_steer(text: &str) -> String {
    format!("@mate {text}\n\nAct on this correction and continue working.")
}

pub fn mate_forced_submit_nudge() -> String {
    "@mate You stopped without submitting. Call mate_submit with a summary of what you accomplished."
        .to_owned()
}

pub fn mate_unaddressed_bounce() -> String {
    "Your last message didn't address anyone. \
     Please start messages with @captain, @human, or @admiral."
        .to_owned()
}

// ---------------------------------------------------------------------------
// Utility wrappers
// ---------------------------------------------------------------------------

pub fn mate_update_interrupt(injected: &str) -> String {
    injected.to_owned()
}
