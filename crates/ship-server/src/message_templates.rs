/// Centralized message templates for captain↔mate communication.
///
/// Organized by audience: captain-facing (about the mate), mate-facing
/// (from the captain/system to the mate), and utility wrappers.

// ---------------------------------------------------------------------------
// Captain-facing messages (injected into the captain's feed)
// ---------------------------------------------------------------------------

pub fn mate_plan_set(plan_status: &str) -> String {
    format!(
        "The mate has set their plan.\n\n\
         {plan_status}\n\n\
         We will keep you posted as they progress. You have nothing to do now."
    )
}

pub fn mate_step_committed(
    step_description: &str,
    commit_summary: &str,
    diff_section: &str,
) -> String {
    format!(
        "The mate completed a step from their plan.\n\n\
         Completed: {step_description}\n\n\
         {commit_summary}{diff_section}\n\n\
         We will notify you when they are done and need your review."
    )
}

pub fn mate_committed_no_step(commit_summary: &str, diff_section: &str) -> String {
    format!("The mate committed without a plan step.\n\n{commit_summary}{diff_section}")
}

pub fn mate_update(message: &str) -> String {
    format!("<mate-update>\n{message}\n</mate-update>")
}

pub fn mate_submitted(summary: &str) -> String {
    format!("The mate has submitted their work for review: {summary}")
}

pub fn mate_question(question: &str) -> String {
    format!("The mate has a question for you: {question}")
}

pub fn mate_activity_summary(summary: &str) -> String {
    format!("<mate-activity-summary>\n{summary}\n</mate-activity-summary>")
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
    format!(
        "<system-notification>\
         Captain steer:\n\
         {text}\n\n\
         Act on this correction and continue working by calling tools. Do not write \
         a text response to the captain — they cannot see your text output.\
         </system-notification>"
    )
}

pub fn mate_forced_submit_nudge() -> String {
    "<system-notification>You stopped without submitting your work. \
     Call mate_submit with a summary of what you accomplished. \
     Do not write a text response — use the mate_submit tool.\
     </system-notification>"
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
    format!("YOUR MATE HAS AN UPDATE:\n\n{injected}")
}
