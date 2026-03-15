use std::path::{Path, PathBuf};

use ship_code::edit::apply_edits;
use ship_code::ops::{Edit, SearchOp};
use ship_code::search::{format_output, search};
use ship_code::symbols::extract_rust_symbols;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sample_project")
}

// ─── Search: text matches ────────────────────────────────────────────

#[test]
fn search_text_exact_string() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "max_connections".to_owned(),
        path: None,
        file_glob: None,
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_case_insensitive() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "SESSION".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_regex_pattern() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: r"fn \w+_session".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_invalid_regex_falls_back_to_literal() {
    // Unbalanced brace — should fall back to literal search
    let output = search(&fixtures_dir(), &SearchOp {
        query: "Self {".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_no_matches() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "this_string_does_not_exist_anywhere".to_owned(),
        path: None,
        file_glob: None,
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_scoped_to_path() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "pub fn".to_owned(),
        path: Some("src/utils.rs".to_owned()),
        file_glob: None,
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_text_across_file_types() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "token".to_owned(),
        path: None,
        file_glob: None,
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

// ─── Search: symbol matches ──────────────────────────────────────────

#[test]
fn search_symbol_function_by_name() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "find_user_sessions".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_struct_by_name() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "ServerConfig".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_impl_block() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "impl Server".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_trait() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "SessionValidator".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_enum() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "ServerError".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_constant() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "MAX_RETRIES".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_substring_fuzzy() {
    // Should find both "Session" struct and "SessionValidator" trait
    let output = search(&fixtures_dir(), &SearchOp {
        query: "Session".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_kind_qualified() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "fn retry".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

#[test]
fn search_symbol_macro() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "log_debug".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!(format_output(&output));
}

// ─── Symbol extraction ───────────────────────────────────────────────

#[test]
fn extract_symbols_server_rs() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/server.rs")).unwrap();
    let symbols = extract_rust_symbols(&source).unwrap();
    let summary: Vec<String> = symbols
        .iter()
        .map(|s| {
            let name = s.name.as_deref().unwrap_or("<unnamed>");
            let parent = match &s.parent {
                Some(p) => format!(" (in {p})"),
                None => String::new(),
            };
            format!(
                "{} {} [{}:{}..{}]{}",
                s.kind, name, "server.rs", s.start_line, s.end_line, parent
            )
        })
        .collect();
    insta::assert_snapshot!(summary.join("\n"));
}

#[test]
fn extract_symbols_utils_rs() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/utils.rs")).unwrap();
    let symbols = extract_rust_symbols(&source).unwrap();
    let summary: Vec<String> = symbols
        .iter()
        .map(|s| {
            let name = s.name.as_deref().unwrap_or("<unnamed>");
            format!("{} {} [{}..{}]", s.kind, name, s.start_line, s.end_line)
        })
        .collect();
    insta::assert_snapshot!(summary.join("\n"));
}

// ─── Edit operations ─────────────────────────────────────────────────

#[test]
fn edit_replace_lines() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/utils.rs")).unwrap();
    let (_, diff) = apply_edits(
        &source,
        Path::new("src/utils.rs"),
        &[Edit::ReplaceLines {
            start: 18,
            end: 22,
            content: "pub fn truncate(s: &str, max_len: usize) -> String {\n    \
                      if s.chars().count() <= max_len {\n        \
                      s.to_owned()\n    \
                      } else {\n        \
                      s.chars().take(max_len).collect::<String>() + \"...\"\n    \
                      }\n\
                      }"
                .to_owned(),
        }],
    )
    .unwrap();
    insta::assert_snapshot!(diff);
}

#[test]
fn edit_insert_lines() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/utils.rs")).unwrap();
    let (_, diff) = apply_edits(
        &source,
        Path::new("src/utils.rs"),
        &[Edit::InsertLines {
            before: 1,
            content: "// Copyright 2026 Bearcove\n// Licensed under MIT\n".to_owned(),
        }],
    )
    .unwrap();
    insta::assert_snapshot!(diff);
}

#[test]
fn edit_delete_lines() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/server.rs")).unwrap();
    let (_, diff) = apply_edits(
        &source,
        Path::new("src/server.rs"),
        &[Edit::DeleteLines {
            start: 107,
            end: 112,
        }],
    )
    .unwrap();
    insta::assert_snapshot!(diff);
}

#[test]
fn edit_find_replace_single() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/server.rs")).unwrap();
    let (_, diff) = apply_edits(
        &source,
        Path::new("src/server.rs"),
        &[Edit::FindReplace {
            find: "max_connections".to_owned(),
            replace: "max_conns".to_owned(),
            replace_all: true,
        }],
    )
    .unwrap();
    insta::assert_snapshot!(diff);
}

#[test]
fn edit_multiple_operations() {
    let source = std::fs::read_to_string(fixtures_dir().join("src/utils.rs")).unwrap();
    let (_, diff) = apply_edits(
        &source,
        Path::new("src/utils.rs"),
        &[
            Edit::FindReplace {
                find: "MAX_RETRIES: u32 = 3".to_owned(),
                replace: "MAX_RETRIES: u32 = 5".to_owned(),
                replace_all: false,
            },
            Edit::FindReplace {
                find: "BUFFER_SIZE: usize = 8192".to_owned(),
                replace: "BUFFER_SIZE: usize = 16384".to_owned(),
                replace_all: false,
            },
        ],
    )
    .unwrap();
    insta::assert_snapshot!(diff);
}

// ─── Combined search + edit scenarios ────────────────────────────────

#[test]
fn scenario_find_and_understand_function() {
    // Agent wants to understand the retry function
    let output = search(&fixtures_dir(), &SearchOp {
        query: "fn retry".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!("find_retry_function", format_output(&output));
}

#[test]
fn scenario_find_all_error_types() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "Error".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!("find_all_errors", format_output(&output));
}

#[test]
fn scenario_find_all_impl_blocks_for_session() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "impl Session".to_owned(),
        path: None,
        file_glob: Some("*.rs".to_owned()),
        case_sensitive: false,
    })
    .unwrap();
    insta::assert_snapshot!("find_session_impls", format_output(&output));
}

#[test]
fn scenario_search_typescript() {
    let output = search(&fixtures_dir(), &SearchOp {
        query: "async".to_owned(),
        path: None,
        file_glob: Some("*.ts".to_owned()),
        case_sensitive: true,
    })
    .unwrap();
    insta::assert_snapshot!("search_typescript_async", format_output(&output));
}
