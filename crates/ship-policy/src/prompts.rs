use sailfish::TemplateOnce;

use crate::{Topology, allowed_mentions};

// ── System prompts ───────────────────────────────────────────────────

#[derive(TemplateOnce)]
#[template(path = "captain.stpl")]
pub struct CaptainPrompt {
    pub captain_name: String,
    pub mate_name: String,
    pub human_name: String,
    pub admiral_name: Option<String>,
    pub state_summary: String,
}

#[derive(TemplateOnce)]
#[template(path = "mate.stpl")]
pub struct MatePrompt {
    pub mate_name: String,
    pub captain_name: String,
    pub human_name: String,
    pub task_description: String,
}

#[derive(TemplateOnce)]
#[template(path = "admiral.stpl")]
pub struct AdmiralPrompt {
    pub admiral_name: String,
    pub human_name: String,
    pub lanes: Vec<LaneInfo>,
}

pub struct LaneInfo {
    pub captain_name: String,
    pub label: String,
    pub status_summary: String,
}

// ── Message wrapping ─────────────────────────────────────────────────

#[derive(TemplateOnce)]
#[template(path = "message_wrap.stpl")]
pub struct MessageWrap {
    pub from_name: String,
    pub text: String,
    pub routing_hint: String,
}

#[derive(TemplateOnce)]
#[template(path = "bounce.stpl")]
pub struct BounceMessage {
    pub allowed_names: Vec<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Build the routing hint for a captain receiving a message.
/// e.g. "Reply to mate: @Jordan · Reply to human: @Amos"
pub fn captain_routing_hint(mate_name: &str, human_name: &str) -> String {
    format!("Reply to mate: @{mate_name} · Reply to human: @{human_name}")
}

/// Build the routing hint for a mate receiving a steer.
pub fn mate_routing_hint() -> String {
    "Act on this correction and continue working.".to_string()
}

/// Wrap a message from one participant to be injected into another's context.
pub fn wrap_message(from_name: &str, text: &str, routing_hint: &str) -> String {
    MessageWrap {
        from_name: from_name.to_string(),
        text: text.to_string(),
        routing_hint: routing_hint.to_string(),
    }
    .render_once()
    .expect("message_wrap template should never fail")
}

/// Generate the bounce message for an unaddressed message.
pub fn bounce_for(topology: &Topology, sender_name: &str) -> Option<String> {
    let sender = topology.find_participant(sender_name)?;
    let allowed = allowed_mentions(topology, sender);
    if allowed.is_empty() {
        return None;
    }
    Some(
        BounceMessage {
            allowed_names: allowed,
        }
        .render_once()
        .expect("bounce template should never fail"),
    )
}
