// Centralized message templates for captain↔mate communication.
//
// Every injected message uses `<message from="...">` for attribution
// and `<routing>` for reply instructions, so agents always know
// who is speaking and how to respond.
// ---------------------------------------------------------------------------
// Routing hints (appended to messages so agents know how to reply)
// ---------------------------------------------------------------------------

const CAPTAIN_ROUTING: &str =
    "<routing>Reply to mate: @mate · Reply to human: @human</routing>";

// ---------------------------------------------------------------------------
// Captain-facing messages (injected into the captain's feed)
// ---------------------------------------------------------------------------

pub fn mate_plan_set(plan_status: &str) -> String {
    format!(
        "<message from=\"mate\">\n\
         I've set my plan.\n\n\
         {plan_status}\n\n\
         I'll keep you posted as I progress.\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_step_committed(
    step_description: &str,
    commit_summary: &str,
    diff_section: &str,
) -> String {
    format!(
        "<message from=\"mate\">\n\
         Completed step: {step_description}\n\n\
         {commit_summary}{diff_section}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_committed_no_step(commit_summary: &str, diff_section: &str) -> String {
    format!(
        "<message from=\"mate\">\n\
         Committed:\n\n\
         {commit_summary}{diff_section}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_update(message: &str) -> String {
    format!(
        "<message from=\"mate\">\n\
         {message}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_submitted(summary: &str) -> String {
    format!(
        "<message from=\"mate\">\n\
         I've submitted my work for review: {summary}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_question(question: &str) -> String {
    format!(
        "<message from=\"mate\">\n\
         {question}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn mate_activity_summary(summary: &str) -> String {
    format!(
        "<message from=\"summarizer\">\n\
         {summary}\n\
         </message>\n\
         {CAPTAIN_ROUTING}"
    )
}

pub fn captain_unaddressed_bounce() -> String {
    "<routing>Your last message didn't address anyone. \
     Reply to mate: @mate · Reply to human: @human</routing>"
        .to_owned()
}

// ---------------------------------------------------------------------------
// Mate-facing messages (injected into the mate's feed)
// ---------------------------------------------------------------------------

pub fn captain_steer(text: &str) -> String {
    format!(
        "<message from=\"captain\">\n\
         {text}\n\
         </message>\n\
         <routing>Act on this correction and continue working.</routing>"
    )
}

pub fn mate_forced_submit_nudge() -> String {
    "<routing>You stopped without submitting. \
         Call mate_submit with a summary of what you accomplished.</routing>"
        .to_string()
}

pub fn mate_unaddressed_bounce() -> String {
    "<routing>Your last message didn't address anyone. \
     Reply to captain: @captain · Reply to human: @human</routing>"
        .to_owned()
}

// ---------------------------------------------------------------------------
// Utility wrappers
// ---------------------------------------------------------------------------

pub fn mate_update_interrupt(injected: &str) -> String {
    injected.to_owned()
}
