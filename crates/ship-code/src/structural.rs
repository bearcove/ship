use std::path::Path;

use eyre::Result;

use crate::symbols::{self, Symbol, SymbolKind, extract_rust_symbols};

/// Result of reading a node from a file.
pub struct ReadNodeResult {
    /// The matched symbol metadata.
    pub kind: SymbolKind,
    pub name: String,
    pub parent: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    /// The source text (potentially windowed by offset/limit).
    pub text: String,
    /// Whether the text was windowed (not the full body).
    pub windowed: bool,
    /// Total number of lines in the full symbol.
    pub total_lines: usize,
}

/// Read a specific symbol from a file by query.
///
/// The query is a symbol name or kind-qualified name like "fn retry"
/// or "impl ShipImpl". If multiple symbols match, returns the best
/// (exact > case-insensitive > substring).
///
/// `offset` and `limit` are 0-indexed line offsets within the symbol body.
pub fn read_node(
    file: &Path,
    source: &str,
    query: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<ReadNodeResult> {
    let symbols = extract_rust_symbols(source)?;
    let matches = symbols::find_symbols(&symbols, query);

    let sym = matches
        .first()
        .ok_or_else(|| eyre::eyre!("no symbol matching {query:?} found in {}", file.display()))?;

    let full_text = sym.text(source);
    let total_lines = sym.line_count();

    let (text, windowed) = match (offset, limit) {
        (Some(off), Some(lim)) => {
            let lines: Vec<&str> = full_text.lines().collect();
            let start = off.min(lines.len());
            let end = (start + lim).min(lines.len());
            let windowed_text: String = lines[start..end].join("\n");
            (windowed_text, true)
        }
        (Some(off), None) => {
            let lines: Vec<&str> = full_text.lines().collect();
            let start = off.min(lines.len());
            let windowed_text: String = lines[start..].join("\n");
            (windowed_text, start > 0)
        }
        (None, Some(lim)) => {
            let lines: Vec<&str> = full_text.lines().collect();
            let end = lim.min(lines.len());
            let windowed_text: String = lines[..end].join("\n");
            (windowed_text, end < lines.len())
        }
        (None, None) => (full_text.to_owned(), false),
    };

    Ok(ReadNodeResult {
        kind: sym.kind,
        name: sym.name.clone().unwrap_or_default(),
        parent: sym.parent.clone(),
        start_line: sym.start_line,
        end_line: sym.end_line,
        text,
        windowed,
        total_lines,
    })
}

/// Replace a symbol's body with new content.
///
/// Re-parses the file to find the symbol fresh (no stale byte offsets).
/// Returns the new file content and a unified diff.
pub fn replace_node(
    file: &Path,
    source: &str,
    query: &str,
    new_content: &str,
) -> Result<(String, String)> {
    let symbols = extract_rust_symbols(source)?;
    let matches = symbols::find_symbols(&symbols, query);

    let sym = matches
        .first()
        .ok_or_else(|| eyre::eyre!("no symbol matching {query:?} found in {}", file.display()))?;

    // Use byte offsets for precise replacement
    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..sym.start_byte]);
    result.push_str(new_content);
    result.push_str(&source[sym.end_byte..]);

    let diff = crate::edit::unified_diff(file, source, &result);
    Ok((result, diff))
}

/// Delete a symbol from a file.
///
/// Re-parses the file to find the symbol fresh.
/// Also removes any blank lines left behind by the deletion.
/// Returns the new file content and a unified diff.
pub fn delete_node(
    file: &Path,
    source: &str,
    query: &str,
) -> Result<(String, String)> {
    let symbols = extract_rust_symbols(source)?;
    let matches = symbols::find_symbols(&symbols, query);

    let sym = matches
        .first()
        .ok_or_else(|| eyre::eyre!("no symbol matching {query:?} found in {}", file.display()))?;

    // Remove the symbol and any trailing newline
    let mut end = sym.end_byte;
    if source.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    // Remove one more newline if it creates a double blank line
    if source.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }

    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..sym.start_byte]);
    result.push_str(&source[end..]);

    let diff = crate::edit::unified_diff(file, source, &result);
    Ok((result, diff))
}

/// Format a symbol's signature (first few lines) for display when the
/// full body is too large.
pub fn symbol_signature(source: &str, sym: &Symbol, max_lines: usize) -> String {
    let text = sym.text(source);
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() <= max_lines {
        return text.to_owned();
    }

    let mut sig: String = lines[..max_lines].join("\n");
    sig.push_str(&format!(
        "\n    // ... {} more lines",
        lines.len() - max_lines
    ));
    sig
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"struct Foo {
    x: i32,
    y: String,
}

impl Foo {
    fn new(x: i32) -> Self {
        Self {
            x,
            y: String::new(),
        }
    }

    fn value(&self) -> i32 {
        self.x
    }
}

fn standalone() {
    println!("hello");
}

const MAX: usize = 100;
"#;

    #[test]
    fn read_node_full() {
        let result = read_node(
            Path::new("test.rs"),
            SAMPLE,
            "fn standalone",
            None,
            None,
        )
        .unwrap();
        assert_eq!(result.name, "standalone");
        assert!(!result.windowed);
        assert!(result.text.contains("println!"));
    }

    #[test]
    fn read_node_with_offset_and_limit() {
        let result = read_node(
            Path::new("test.rs"),
            SAMPLE,
            "impl Foo",
            Some(1),
            Some(3),
        )
        .unwrap();
        assert!(result.windowed);
        // Should show 3 lines starting from offset 1 (skipping the `impl Foo {` line)
        assert!(!result.text.starts_with("impl"));
    }

    #[test]
    fn read_node_not_found() {
        let result = read_node(
            Path::new("test.rs"),
            SAMPLE,
            "fn nonexistent",
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn replace_node_function() {
        let (new_source, diff) = replace_node(
            Path::new("test.rs"),
            SAMPLE,
            "fn standalone",
            "fn standalone() {\n    println!(\"replaced!\");\n}",
        )
        .unwrap();
        assert!(new_source.contains("replaced!"));
        assert!(!new_source.contains("hello"));
        assert!(!diff.is_empty());
        // Other symbols should be untouched
        assert!(new_source.contains("struct Foo"));
        assert!(new_source.contains("const MAX"));
    }

    #[test]
    fn replace_node_struct() {
        let (new_source, _) = replace_node(
            Path::new("test.rs"),
            SAMPLE,
            "struct Foo",
            "struct Foo {\n    x: i32,\n    y: String,\n    z: bool,\n}",
        )
        .unwrap();
        assert!(new_source.contains("z: bool"));
        // impl should still be there
        assert!(new_source.contains("impl Foo"));
    }

    #[test]
    fn delete_node_function() {
        let (new_source, diff) = delete_node(
            Path::new("test.rs"),
            SAMPLE,
            "fn standalone",
        )
        .unwrap();
        assert!(!new_source.contains("fn standalone"));
        assert!(!new_source.contains("println!(\"hello\")"));
        // Other symbols should be untouched
        assert!(new_source.contains("struct Foo"));
        assert!(new_source.contains("const MAX"));
        assert!(!diff.is_empty());
    }

    #[test]
    fn delete_node_constant() {
        let (new_source, _) = delete_node(
            Path::new("test.rs"),
            SAMPLE,
            "const MAX",
        )
        .unwrap();
        assert!(!new_source.contains("const MAX"));
        assert!(new_source.contains("fn standalone"));
    }

    #[test]
    fn replace_node_method_inside_impl() {
        let (new_source, _) = replace_node(
            Path::new("test.rs"),
            SAMPLE,
            "fn value",
            "fn value(&self) -> i32 {\n        self.x * 2\n    }",
        )
        .unwrap();
        assert!(new_source.contains("self.x * 2"));
        assert!(!new_source.contains("self.x\n"));
        // The impl block and other method should still be intact
        assert!(new_source.contains("fn new"));
        assert!(new_source.contains("impl Foo"));
    }

    #[test]
    fn symbol_signature_short() {
        let symbols = crate::symbols::extract_rust_symbols(SAMPLE).unwrap();
        let standalone = symbols.iter().find(|s| s.name.as_deref() == Some("standalone")).unwrap();
        let sig = symbol_signature(SAMPLE, standalone, 10);
        // Short function, should show the whole thing
        assert!(sig.contains("println!"));
        assert!(!sig.contains("more lines"));
    }

    #[test]
    fn symbol_signature_long() {
        let symbols = crate::symbols::extract_rust_symbols(SAMPLE).unwrap();
        let impl_foo = symbols.iter().find(|s| s.name.as_deref() == Some("Foo") && s.kind == SymbolKind::Impl).unwrap();
        let sig = symbol_signature(SAMPLE, impl_foo, 3);
        // Should truncate after 3 lines
        assert!(sig.contains("more lines"));
    }
}
