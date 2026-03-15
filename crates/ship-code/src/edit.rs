use eyre::{Result, bail};
use similar::TextDiff;
use std::path::Path;

use crate::ops::Edit;

/// Apply a list of edits to a file's content. Returns the new content
/// and a unified diff.
pub fn apply_edits(source: &str, path: &Path, edits: &[Edit]) -> Result<(String, String)> {
    let mut content = source.to_owned();

    for edit in edits {
        content = apply_single_edit(&content, edit)?;
    }

    let diff = unified_diff(path, source, &content);
    Ok((content, diff))
}

fn apply_single_edit(source: &str, edit: &Edit) -> Result<String> {
    match edit {
        Edit::ReplaceLines {
            start,
            end,
            content,
        } => replace_lines(source, *start, *end, content),
        Edit::InsertLines { before, content } => insert_lines(source, *before, content),
        Edit::DeleteLines { start, end } => delete_lines(source, *start, *end),
        Edit::FindReplace {
            find,
            replace,
            replace_all,
        } => find_replace(source, find, replace, *replace_all),
    }
}

/// Replace lines start..=end (1-indexed, inclusive) with new content.
fn replace_lines(source: &str, start: usize, end: usize, content: &str) -> Result<String> {
    if start == 0 {
        bail!("line numbers are 1-indexed, got start=0");
    }
    if end < start {
        bail!("end ({end}) must be >= start ({start})");
    }

    let lines: Vec<&str> = source.lines().collect();
    let total = lines.len();

    if start > total + 1 {
        bail!("start line {start} is beyond end of file ({total} lines)");
    }

    let mut result = String::with_capacity(source.len());

    // Lines before the replacement (1-indexed, so start-1 in 0-indexed)
    for line in &lines[..start - 1] {
        result.push_str(line);
        result.push('\n');
    }

    // Insert the replacement content
    if !content.is_empty() {
        result.push_str(content);
        if !content.ends_with('\n') {
            result.push('\n');
        }
    }

    // Lines after the replacement
    let skip_end = end.min(total);
    for line in &lines[skip_end..] {
        result.push_str(line);
        result.push('\n');
    }

    // Preserve trailing newline behavior of original
    if !source.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    Ok(result)
}

/// Insert content before a specific line (1-indexed).
fn insert_lines(source: &str, before: usize, content: &str) -> Result<String> {
    if before == 0 {
        bail!("line numbers are 1-indexed, got before=0");
    }

    let lines: Vec<&str> = source.lines().collect();

    let mut result = String::with_capacity(source.len() + content.len());

    let insert_at = (before - 1).min(lines.len());

    for line in &lines[..insert_at] {
        result.push_str(line);
        result.push('\n');
    }

    result.push_str(content);
    if !content.ends_with('\n') {
        result.push('\n');
    }

    for line in &lines[insert_at..] {
        result.push_str(line);
        result.push('\n');
    }

    if !source.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    Ok(result)
}

/// Delete lines start..=end (1-indexed, inclusive).
fn delete_lines(source: &str, start: usize, end: usize) -> Result<String> {
    replace_lines(source, start, end, "")
}

/// Find and replace text.
fn find_replace(source: &str, find: &str, replace: &str, replace_all: bool) -> Result<String> {
    if !source.contains(find) {
        bail!("text not found: {find:?}");
    }

    if replace_all {
        Ok(source.replace(find, replace))
    } else {
        Ok(source.replacen(find, replace, 1))
    }
}

/// Generate a unified diff between old and new content.
fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }

    let path_str = path.display();
    let diff = TextDiff::from_lines(old, new);

    let mut output = String::new();
    output.push_str(&format!("--- a/{path_str}\n"));
    output.push_str(&format!("+++ b/{path_str}\n"));

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&format!("{hunk}"));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "line 1\nline 2\nline 3\nline 4\nline 5\n";

    #[test]
    fn replace_middle_lines() {
        let edits = vec![Edit::ReplaceLines {
            start: 2,
            end: 3,
            content: "replaced A\nreplaced B".to_owned(),
        }];
        let (result, diff) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        assert!(result.contains("replaced A"));
        assert!(result.contains("replaced B"));
        assert!(!result.contains("line 2"));
        assert!(!result.contains("line 3"));
        assert!(result.contains("line 1"));
        assert!(result.contains("line 4"));
        assert!(!diff.is_empty());
    }

    #[test]
    fn insert_before_line() {
        let edits = vec![Edit::InsertLines {
            before: 3,
            content: "inserted".to_owned(),
        }];
        let (result, _) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[2], "inserted");
        assert_eq!(lines[3], "line 3");
    }

    #[test]
    fn delete_lines() {
        let edits = vec![Edit::DeleteLines { start: 2, end: 4 }];
        let (result, _) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[1], "line 5");
    }

    #[test]
    fn find_replace_single() {
        let edits = vec![Edit::FindReplace {
            find: "line 3".to_owned(),
            replace: "LINE THREE".to_owned(),
            replace_all: false,
        }];
        let (result, _) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        assert!(result.contains("LINE THREE"));
        assert!(!result.contains("line 3"));
    }

    #[test]
    fn find_replace_all() {
        let edits = vec![Edit::FindReplace {
            find: "line".to_owned(),
            replace: "LINE".to_owned(),
            replace_all: true,
        }];
        let (result, _) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        assert_eq!(result.matches("LINE").count(), 5);
        assert_eq!(result.matches("line").count(), 0);
    }

    #[test]
    fn find_replace_not_found() {
        let edits = vec![Edit::FindReplace {
            find: "nonexistent".to_owned(),
            replace: "whatever".to_owned(),
            replace_all: false,
        }];
        let result = apply_edits(SAMPLE, Path::new("test.rs"), &edits);
        assert!(result.is_err());
    }

    #[test]
    fn diff_output_has_headers() {
        let edits = vec![Edit::ReplaceLines {
            start: 2,
            end: 2,
            content: "changed".to_owned(),
        }];
        let (_, diff) = apply_edits(SAMPLE, Path::new("src/foo.rs"), &edits).unwrap();
        assert!(diff.contains("--- a/src/foo.rs"));
        assert!(diff.contains("+++ b/src/foo.rs"));
        assert!(diff.contains("-line 2"));
        assert!(diff.contains("+changed"));
    }

    #[test]
    fn multiple_edits_applied_in_order() {
        let edits = vec![
            Edit::FindReplace {
                find: "line 1".to_owned(),
                replace: "FIRST".to_owned(),
                replace_all: false,
            },
            Edit::FindReplace {
                find: "line 5".to_owned(),
                replace: "LAST".to_owned(),
                replace_all: false,
            },
        ];
        let (result, _) = apply_edits(SAMPLE, Path::new("test.rs"), &edits).unwrap();
        assert!(result.contains("FIRST"));
        assert!(result.contains("LAST"));
    }

    #[test]
    fn zero_line_number_errors() {
        let edits = vec![Edit::ReplaceLines {
            start: 0,
            end: 1,
            content: "bad".to_owned(),
        }];
        assert!(apply_edits(SAMPLE, Path::new("test.rs"), &edits).is_err());
    }
}
