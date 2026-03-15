use std::path::Path;

use eyre::{Result, bail};
use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::SearcherBuilder;

use crate::ops::{SearchOp, TextMatch};
use crate::symbols;

/// Result of a search operation, combining text matches and symbol matches.
pub struct SearchOutput {
    pub text_matches: Vec<TextMatch>,
    pub symbol_matches: Vec<SymbolMatch>,
}

/// A symbol match from the tree-sitter index.
pub struct SymbolMatch {
    pub kind: String,
    pub name: String,
    pub parent: Option<String>,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Source text if under the line threshold, otherwise None.
    pub body: Option<String>,
}

const SYMBOL_BODY_MAX_LINES: usize = 50;

/// Execute a search operation against a worktree.
pub fn search(worktree: &Path, op: &SearchOp) -> Result<SearchOutput> {
    let search_root = match &op.path {
        Some(p) => worktree.join(p),
        None => worktree.to_owned(),
    };

    let text_matches = search_text(&search_root, &op.query, op.case_sensitive, op.file_glob.as_deref())?;
    let symbol_matches = search_symbols(worktree, &op.query, op.file_glob.as_deref())?;

    Ok(SearchOutput {
        text_matches,
        symbol_matches,
    })
}

/// Search for text matches using ripgrep's library.
/// Tries the query as a regex first, falls back to literal.
fn search_text(
    root: &Path,
    query: &str,
    case_sensitive: bool,
    file_glob: Option<&str>,
) -> Result<Vec<TextMatch>> {
    let matcher = build_matcher(query, case_sensitive)?;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(2)
        .after_context(2)
        .build();

    let mut all_matches = Vec::new();

    walk_files(root, file_glob, &mut |path| {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Collect context lines and matches for this file
        let mut file_matches: Vec<(u64, String)> = Vec::new();

        let result = searcher.search_path(
            &matcher,
            path,
            UTF8(|line_num, line| {
                // Check if this line actually contains a match
                if matcher.is_match(line.as_bytes())? {
                    file_matches.push((line_num, line.trim_end().to_owned()));
                }
                Ok(true)
            }),
        );

        if let Err(e) = result {
            tracing::debug!("search error in {}: {e}", path.display());
            return;
        }

        for (line_num, text) in file_matches {
            all_matches.push(TextMatch {
                file: relative.clone(),
                line: line_num as usize,
                text,
                context_before: Vec::new(),
                context_after: Vec::new(),
            });
        }
    })?;

    Ok(all_matches)
}

/// Build a regex matcher, trying the query as-is first (regex),
/// then falling back to treating it as a literal string.
fn build_matcher(
    query: &str,
    case_sensitive: bool,
) -> Result<grep::regex::RegexMatcher> {
    // Try as regex first
    let result = RegexMatcherBuilder::new()
        .case_insensitive(!case_sensitive)
        .build(query);

    match result {
        Ok(m) => Ok(m),
        Err(_) => {
            // Fall back to literal (escape all regex metacharacters)
            let escaped = regex_syntax::escape(query);
            RegexMatcherBuilder::new()
                .case_insensitive(!case_sensitive)
                .build(&escaped)
                .map_err(|e| eyre::eyre!("failed to build matcher for {query:?}: {e}"))
        }
    }
}

/// Search the tree-sitter symbol index for matching symbols.
fn search_symbols(
    worktree: &Path,
    query: &str,
    file_glob: Option<&str>,
) -> Result<Vec<SymbolMatch>> {
    let mut all_symbols = Vec::new();

    walk_files(worktree, file_glob, &mut |path| {
        // Only parse Rust files for now
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("rs") {
            return;
        }

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return,
        };

        let symbols = match symbols::extract_rust_symbols(&source) {
            Ok(s) => s,
            Err(_) => return,
        };

        let matches = symbols::find_symbols(&symbols, query);

        let relative = path
            .strip_prefix(worktree)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        for sym in matches {
            let body = if sym.line_count() <= SYMBOL_BODY_MAX_LINES {
                Some(sym.text(&source).to_owned())
            } else {
                None
            };

            all_symbols.push(SymbolMatch {
                kind: sym.kind.to_string(),
                name: sym.name.clone().unwrap_or_default(),
                parent: sym.parent.clone(),
                file: relative.clone(),
                start_line: sym.start_line,
                end_line: sym.end_line,
                body,
            });
        }
    })?;

    Ok(all_symbols)
}

/// Walk files under a root, optionally filtered by glob, calling f for each.
fn walk_files(
    root: &Path,
    file_glob: Option<&str>,
    f: &mut dyn FnMut(&Path),
) -> Result<()> {
    if !root.exists() {
        bail!("path does not exist: {}", root.display());
    }

    if root.is_file() {
        f(root);
        return Ok(());
    }

    walk_dir_recursive(root, file_glob, f);
    Ok(())
}

fn walk_dir_recursive(dir: &Path, file_glob: Option<&str>, f: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files/dirs and common non-source dirs
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            walk_dir_recursive(&path, file_glob, f);
        } else if path.is_file() {
            if let Some(glob) = file_glob {
                if !matches_glob(&path, glob) {
                    continue;
                }
            }
            f(&path);
        }
    }
}

/// Simple glob matching — supports "*.rs", "*.{rs,toml}", etc.
fn matches_glob(path: &Path, glob: &str) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    // Handle "*.ext" pattern
    if let Some(ext_pattern) = glob.strip_prefix("*.") {
        // Handle "{rs,toml}" brace expansion
        if ext_pattern.starts_with('{') && ext_pattern.ends_with('}') {
            let inner = &ext_pattern[1..ext_pattern.len() - 1];
            return inner
                .split(',')
                .any(|ext| file_name.ends_with(&format!(".{ext}")));
        }
        return file_name.ends_with(&format!(".{ext_pattern}"));
    }

    // Handle "**/*.ext" pattern
    if let Some(rest) = glob.strip_prefix("**/") {
        return matches_glob(path, rest);
    }

    // Exact filename match
    file_name == glob
}

/// Format search output as a human-readable string for the agent.
pub fn format_output(output: &SearchOutput) -> String {
    let mut result = String::new();

    if !output.symbol_matches.is_empty() {
        result.push_str("═══ Symbol matches ═══\n");
        for sym in &output.symbol_matches {
            let parent_info = match &sym.parent {
                Some(p) => format!("  (in {p})"),
                None => String::new(),
            };
            result.push_str(&format!(
                "{} {}  {}:{}-{}{}\n",
                sym.kind, sym.name, sym.file, sym.start_line, sym.end_line, parent_info
            ));
            if let Some(body) = &sym.body {
                result.push_str("```\n");
                result.push_str(body);
                if !body.ends_with('\n') {
                    result.push('\n');
                }
                result.push_str("```\n");
            } else {
                result.push_str(&format!(
                    "({} lines — use read_node to see full body)\n",
                    sym.end_line - sym.start_line + 1
                ));
            }
            result.push('\n');
        }
    }

    if !output.text_matches.is_empty() {
        result.push_str("═══ Text matches ═══\n");
        for m in &output.text_matches {
            result.push_str(&format!("{}:{}: {}\n", m.file, m.line, m.text));
        }
    }

    if output.symbol_matches.is_empty() && output.text_matches.is_empty() {
        result.push_str("No matches found.\n");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();

        fs::write(
            dir.path().join("main.rs"),
            r#"
fn hello_world() {
    println!("Hello, world!");
}

struct Config {
    name: String,
    value: i32,
}

impl Config {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: 0,
        }
    }
}
"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("utils.rs"),
            r#"
fn helper_function() -> bool {
    true
}

const MAX_RETRIES: u32 = 3;
"#,
        )
        .unwrap();

        fs::write(dir.path().join("readme.txt"), "This is not a Rust file.\n")
            .unwrap();

        dir
    }

    #[test]
    fn text_search_finds_matches() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "println".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(
            !output.text_matches.is_empty(),
            "expected text matches for 'println'"
        );
        assert_eq!(output.text_matches[0].file, "main.rs");
    }

    #[test]
    fn text_search_case_insensitive() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "HELLO".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(!output.text_matches.is_empty());
    }

    #[test]
    fn text_search_case_sensitive_misses() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "HELLO".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: true,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(output.text_matches.is_empty());
    }

    #[test]
    fn symbol_search_finds_function() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "hello_world".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(
            !output.symbol_matches.is_empty(),
            "expected symbol match for hello_world"
        );
        assert_eq!(output.symbol_matches[0].kind, "fn");
        assert_eq!(output.symbol_matches[0].name, "hello_world");
    }

    #[test]
    fn symbol_search_finds_struct() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "Config".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        let struct_match = output
            .symbol_matches
            .iter()
            .find(|s| s.kind == "struct");
        assert!(struct_match.is_some(), "expected struct Config match");
    }

    #[test]
    fn symbol_search_includes_body_for_small_symbols() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "helper_function".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(!output.symbol_matches.is_empty());
        assert!(
            output.symbol_matches[0].body.is_some(),
            "small function should include body"
        );
    }

    #[test]
    fn file_glob_filters_non_rust() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "This is not".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(
            output.text_matches.is_empty(),
            "should not find matches in .txt with *.rs glob"
        );
    }

    #[test]
    fn regex_fallback_to_literal() {
        let dir = setup_test_dir();
        // This is invalid regex (unbalanced paren) — should fall back to literal
        let op = SearchOp {
            query: "Self {".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        assert!(
            !output.text_matches.is_empty(),
            "should find literal match after regex fallback"
        );
    }

    #[test]
    fn format_output_includes_both_sections() {
        let dir = setup_test_dir();
        let op = SearchOp {
            query: "Config".to_owned(),
            path: None,
            file_glob: Some("*.rs".to_owned()),
            case_sensitive: false,
        };
        let output = search(dir.path(), &op).unwrap();
        let formatted = format_output(&output);
        assert!(formatted.contains("Symbol matches"));
        assert!(formatted.contains("Text matches"));
    }
}
