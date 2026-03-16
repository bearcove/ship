use crate::{ParticipantName, Topology};

/// Result of parsing a mention from the start of a text block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedMention {
    /// Found `@Name rest-of-text` where Name is a known participant.
    Found { name: ParticipantName, rest: String },
    /// No mention found — text doesn't start with @.
    None,
    /// Starts with @ but doesn't match any known participant.
    Unknown { attempted: String, rest: String },
    /// Looks like a partial mention being streamed (e.g. just "@" or "@Jor").
    /// Don't bounce these.
    Incomplete,
}

/// Parse a mention from the start of a text block, matching against
/// participants known in the topology.
pub fn parse_mention(text: &str, topology: &Topology) -> ParsedMention {
    let trimmed = text.trim_start();

    if !trimmed.starts_with('@') {
        return ParsedMention::None;
    }

    // Skip the @
    let after_at = &trimmed[1..];

    // If there's nothing after @ or no space yet, it's incomplete (streaming)
    if after_at.is_empty() {
        return ParsedMention::Incomplete;
    }

    // Find the end of the mention word (first whitespace or end of string)
    let word_end = after_at
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(after_at.len());
    let mention_word = &after_at[..word_end];

    // If no whitespace found and the text is short, could be incomplete streaming
    if word_end == after_at.len() && !after_at.contains(char::is_whitespace) {
        // Check if this is a complete name match — if so it's valid even without trailing space
        if let Some(participant) = topology.find_participant_ci(mention_word) {
            return ParsedMention::Found {
                name: participant.name.clone(),
                rest: String::new(),
            };
        }
        // Check if any name starts with this prefix — if so, still streaming
        if topology.any_name_starts_with(mention_word) {
            return ParsedMention::Incomplete;
        }
        // Doesn't match anything — unknown mention
        return ParsedMention::Unknown {
            attempted: mention_word.to_string(),
            rest: String::new(),
        };
    }

    let rest = after_at[word_end..].trim_start();

    // Try case-insensitive match against all participants
    if let Some(participant) = topology.find_participant_ci(mention_word) {
        return ParsedMention::Found {
            name: participant.name.clone(),
            rest: rest.to_string(),
        };
    }

    ParsedMention::Unknown {
        attempted: mention_word.to_string(),
        rest: rest.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn topo() -> Topology {
        Topology {
            human: Participant::human("Amos"),
            admiral: Participant::agent("Morgan", AgentRole::Admiral),
            lanes: vec![Lane {
                id: RoomId::from_static("lane-1"),
                captain: Participant::agent("Cedar", AgentRole::Captain),
                mate: Participant::agent("Jordan", AgentRole::Mate),
            }],
        }
    }

    #[test]
    fn parses_mention_at_start() {
        let result = parse_mention("@Cedar fix the tests", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Cedar"),
                rest: "fix the tests".into()
            }
        );
    }

    #[test]
    fn parses_mention_case_insensitive() {
        let result = parse_mention("@cedar fix the tests", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Cedar"),
                rest: "fix the tests".into()
            }
        );
    }

    #[test]
    fn parses_mention_with_leading_whitespace() {
        let result = parse_mention("  @Jordan done", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Jordan"),
                rest: "done".into()
            }
        );
    }

    #[test]
    fn parses_human_mention() {
        let result = parse_mention("@Amos task is ready", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Amos"),
                rest: "task is ready".into()
            }
        );
    }

    #[test]
    fn parses_admiral_mention() {
        let result = parse_mention("@Morgan reassign lane 2", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Morgan"),
                rest: "reassign lane 2".into()
            }
        );
    }

    #[test]
    fn no_mention() {
        let result = parse_mention("just some text", &topo());
        assert_eq!(result, ParsedMention::None);
    }

    #[test]
    fn empty_text() {
        let result = parse_mention("", &topo());
        assert_eq!(result, ParsedMention::None);
    }

    #[test]
    fn bare_at_sign_is_incomplete() {
        let result = parse_mention("@", &topo());
        assert_eq!(result, ParsedMention::Incomplete);
    }

    #[test]
    fn partial_name_is_incomplete() {
        let result = parse_mention("@Ced", &topo());
        assert_eq!(result, ParsedMention::Incomplete);
    }

    #[test]
    fn unknown_mention() {
        let result = parse_mention("@Nobody help me", &topo());
        assert_eq!(
            result,
            ParsedMention::Unknown {
                attempted: "Nobody".into(),
                rest: "help me".into()
            }
        );
    }

    #[test]
    fn unknown_mention_no_prefix_match() {
        let result = parse_mention("@Zzz", &topo());
        assert_eq!(
            result,
            ParsedMention::Unknown {
                attempted: "Zzz".into(),
                rest: String::new()
            }
        );
    }

    #[test]
    fn mention_with_no_body() {
        let result = parse_mention("@Cedar", &topo());
        assert_eq!(
            result,
            ParsedMention::Found {
                name: ParticipantName::from_static("Cedar"),
                rest: String::new()
            }
        );
    }

    #[test]
    fn mention_at_in_middle_is_no_mention() {
        // @ not at start — no mention
        let result = parse_mention("hello @Cedar", &topo());
        assert_eq!(result, ParsedMention::None);
    }
}
