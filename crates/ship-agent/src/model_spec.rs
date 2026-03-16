use ship_types::AgentKind;

/// Parsed model spec, e.g. `claude::opus` → kind=Claude, model="opus".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpec {
    pub kind: AgentKind,
    pub model: String,
}

impl ModelSpec {
    /// Parse a spec like `claude::opus` or `codex::gpt-5.4-high`.
    pub fn parse(s: &str) -> Option<Self> {
        let (kind_str, model) = s.split_once("::")?;
        let kind = match kind_str {
            "claude" => AgentKind::Claude,
            "codex" => AgentKind::Codex,
            "opencode" => AgentKind::OpenCode,
            _ => return None,
        };
        if model.is_empty() {
            return None;
        }
        Some(Self {
            kind,
            model: model.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_claude_opus() {
        let spec = ModelSpec::parse("claude::opus").unwrap();
        assert_eq!(spec.kind, AgentKind::Claude);
        assert_eq!(spec.model, "opus");
    }

    #[test]
    fn parse_codex_model() {
        let spec = ModelSpec::parse("codex::gpt-5.4-high").unwrap();
        assert_eq!(spec.kind, AgentKind::Codex);
        assert_eq!(spec.model, "gpt-5.4-high");
    }

    #[test]
    fn parse_opencode_model() {
        let spec = ModelSpec::parse("opencode::glm-5").unwrap();
        assert_eq!(spec.kind, AgentKind::OpenCode);
        assert_eq!(spec.model, "glm-5");
    }

    #[test]
    fn reject_unknown_kind() {
        assert!(ModelSpec::parse("gemini::pro").is_none());
    }

    #[test]
    fn reject_missing_model() {
        assert!(ModelSpec::parse("claude::").is_none());
    }

    #[test]
    fn reject_no_separator() {
        assert!(ModelSpec::parse("claude-opus").is_none());
    }

    #[test]
    fn reject_single_colon() {
        assert!(ModelSpec::parse("claude:opus").is_none());
    }
}
