use crate::{AgentRole, ParticipantKind, Topology, allowed_mentions};

/// A message from one participant mentioning another.
#[derive(Debug, Clone)]
pub struct Message {
    pub from: String,
    pub mention: String,
    pub text: String,
}

/// What should happen when a message is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResult {
    /// Deliver the message to the target participant.
    Deliver {
        to: String,
        text: String,
    },
    /// The mention is "@human" from a captain — intercept and deliver to admiral instead.
    InterceptForAdmiral {
        original_target: String,
        to_admiral: String,
        from_captain: String,
        text: String,
    },
    /// The sender is not allowed to mention this target.
    Denied {
        from: String,
        attempted_target: String,
        reason: String,
    },
    /// The mentioned name doesn't exist in the topology.
    UnknownTarget {
        from: String,
        attempted_target: String,
    },
    /// No mention found — bounce back to sender.
    Unaddressed {
        from: String,
        text: String,
    },
}

/// Route a message through the topology, applying access control and interception.
pub fn route_message(topology: &Topology, message: &Message) -> RouteResult {
    // Find the sender
    let sender = match topology.find_participant(&message.from) {
        Some(p) => p,
        None => {
            return RouteResult::UnknownTarget {
                from: message.from.clone(),
                attempted_target: message.mention.clone(),
            };
        }
    };

    // Empty mention = unaddressed
    if message.mention.is_empty() {
        return RouteResult::Unaddressed {
            from: message.from.clone(),
            text: message.text.clone(),
        };
    }

    // Check the target exists
    let target = match topology.find_participant(&message.mention) {
        Some(p) => p,
        None => {
            return RouteResult::UnknownTarget {
                from: message.from.clone(),
                attempted_target: message.mention.clone(),
            };
        }
    };

    // Check access control
    let allowed = allowed_mentions(topology, sender);
    if !allowed.contains(&message.mention) {
        return RouteResult::Denied {
            from: message.from.clone(),
            attempted_target: message.mention.clone(),
            reason: format!(
                "{} ({:?}) cannot mention {}",
                sender.name,
                sender.kind,
                target.name
            ),
        };
    }

    // Interception: captain says @human → goes to admiral
    if matches!(sender.kind, ParticipantKind::Agent(AgentRole::Captain)) && target.is_human() {
        return RouteResult::InterceptForAdmiral {
            original_target: target.name.clone(),
            to_admiral: topology.admiral.name.clone(),
            from_captain: sender.name.clone(),
            text: message.text.clone(),
        };
    }

    // Normal delivery
    RouteResult::Deliver {
        to: target.name.clone(),
        text: message.text.clone(),
    }
}
