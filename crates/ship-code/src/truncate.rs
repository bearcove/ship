use std::sync::LazyLock;

use tiktoken_rs::CoreBPE;

/// Maximum tokens we allow in a tool response before truncating.
/// ACP truncates at 25000 tokens — we stay well under to leave room
/// for our trailing instructions.
const MAX_OUTPUT_TOKENS: usize = 20_000;

static TOKENIZER: LazyLock<CoreBPE> = LazyLock::new(|| {
    tiktoken_rs::cl100k_base().expect("failed to load cl100k_base tokenizer")
});

/// Count the number of tokens in a string using cl100k_base.
pub fn count_tokens(text: &str) -> usize {
    TOKENIZER.encode_ordinary(text).len()
}

/// Truncate text to fit within the token budget.
/// If truncated, appends a message indicating how much was cut.
pub fn truncate_output(text: &str, suffix: &str) -> String {
    let total = count_tokens(text);
    if total <= MAX_OUTPUT_TOKENS {
        return text.to_owned();
    }

    // Binary search for the right byte position that fits in the budget.
    // Reserve tokens for the truncation message + suffix.
    let reserved = 100; // tokens for the truncation notice
    let target_tokens = MAX_OUTPUT_TOKENS - reserved;

    let truncated = truncate_to_tokens(text, target_tokens);
    let remaining = total - count_tokens(&truncated);

    format!(
        "{truncated}\n\n\
         (output truncated — ~{remaining} tokens omitted. Narrow your search.)\n\
         {suffix}"
    )
}

/// Truncate a string to approximately `max_tokens` tokens,
/// cutting at the last newline boundary to avoid breaking lines.
fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let tokens = TOKENIZER.encode_ordinary(text);
    if tokens.len() <= max_tokens {
        return text.to_owned();
    }

    // Decode the first max_tokens tokens back to text
    let truncated = TOKENIZER
        .decode(tokens[..max_tokens].to_vec())
        .unwrap_or_default();

    // Cut at the last newline to avoid partial lines
    match truncated.rfind('\n') {
        Some(pos) => truncated[..pos].to_owned(),
        None => truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_tokens_basic() {
        let count = count_tokens("hello world");
        assert!(count > 0);
        assert!(count < 10);
    }

    #[test]
    fn short_text_not_truncated() {
        let text = "hello world";
        let result = truncate_output(text, "");
        assert_eq!(result, text);
    }

    #[test]
    fn long_text_is_truncated() {
        // Generate text that's definitely over the token limit
        let line = "This is a line of text that will be repeated many times to exceed the token limit.\n";
        let text = line.repeat(10_000);
        let result = truncate_output(&text, "<routing>Reply to captain: @captain</routing>");

        assert!(count_tokens(&result) < 25_000);
        assert!(result.contains("output truncated"));
        assert!(result.contains("Narrow your search"));
        assert!(result.contains("<routing>"));
    }
}
