use arborium_tree_sitter::{Node, Parser};
use eyre::Result;

/// A semantic code symbol extracted from source via tree-sitter.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// What kind of symbol this is.
    pub kind: SymbolKind,
    /// The symbol's name (e.g. "ShipImpl", "notify_captain_progress").
    pub name: Option<String>,
    /// For methods: the parent impl/trait name.
    pub parent: Option<String>,
    /// 1-indexed start line (inclusive).
    pub start_line: usize,
    /// 1-indexed end line (inclusive).
    pub end_line: usize,
    /// Byte offset of start in source.
    pub start_byte: usize,
    /// Byte offset of end in source.
    pub end_byte: usize,
}

impl Symbol {
    /// Number of lines this symbol spans.
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Extract the source text for this symbol.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start_byte..self.end_byte]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Const,
    Static,
    TypeAlias,
    Macro,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "fn"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Impl => write!(f, "impl"),
            Self::Module => write!(f, "mod"),
            Self::Const => write!(f, "const"),
            Self::Static => write!(f, "static"),
            Self::TypeAlias => write!(f, "type"),
            Self::Macro => write!(f, "macro"),
        }
    }
}

/// Parse Rust source and extract all top-level and nested symbols.
pub fn extract_rust_symbols(source: &str) -> Result<Vec<Symbol>> {
    let mut parser = Parser::new();
    parser
        .set_language(&arborium_rust::language().into())
        .map_err(|e| eyre::eyre!("failed to set Rust language: {e}"))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| eyre::eyre!("tree-sitter parse failed"))?;

    let mut symbols = Vec::new();
    extract_recursive(source, tree.root_node(), None, &mut symbols);
    Ok(symbols)
}

fn extract_recursive(
    source: &str,
    node: Node,
    parent_name: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let kind = match rust_node_kind(node.kind()) {
        Some(k) => k,
        None => {
            // Not a symbol node — recurse into children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                extract_recursive(source, child, parent_name, symbols);
            }
            return;
        }
    };

    let name = get_node_name(source, node);
    let current_name = name.clone();

    symbols.push(Symbol {
        kind,
        name,
        parent: parent_name.map(str::to_owned),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    });

    // Recurse into children (e.g. methods inside impl blocks)
    let child_parent = current_name.as_deref().or(parent_name);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_recursive(source, child, child_parent, symbols);
    }
}

/// Map tree-sitter node kind strings to our SymbolKind.
fn rust_node_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_item" => Some(SymbolKind::Function),
        "struct_item" => Some(SymbolKind::Struct),
        "enum_item" => Some(SymbolKind::Enum),
        "trait_item" => Some(SymbolKind::Trait),
        "impl_item" => Some(SymbolKind::Impl),
        "mod_item" => Some(SymbolKind::Module),
        "const_item" => Some(SymbolKind::Const),
        "static_item" => Some(SymbolKind::Static),
        "type_item" => Some(SymbolKind::TypeAlias),
        "macro_definition" => Some(SymbolKind::Macro),
        _ => None,
    }
}

/// Extract the name of a node from its "name" or "type" field.
fn get_node_name(source: &str, node: Node) -> Option<String> {
    // Try "name" field first (functions, structs, enums, traits, modules, etc.)
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(source[name_node.byte_range()].to_string());
    }

    // For impl blocks, try "type" field (e.g. `impl ShipImpl`)
    if let Some(type_node) = node.child_by_field_name("type") {
        let type_text = source[type_node.byte_range()].to_string();
        // Also check for trait impl: `impl Trait for Type`
        if let Some(trait_node) = node.child_by_field_name("trait") {
            let trait_text = source[trait_node.byte_range()].to_string();
            return Some(format!("{trait_text} for {type_text}"));
        }
        return Some(type_text);
    }

    // Fallback: first identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(source[child.byte_range()].to_string());
        }
    }

    None
}

/// Find symbols matching a query string.
/// Supports:
/// - Exact name match: "notify_captain_progress"
/// - Kind-qualified: "fn notify_captain_progress", "impl ShipImpl"
/// - Fuzzy substring match on name
pub fn find_symbols<'a>(symbols: &'a [Symbol], query: &str) -> Vec<&'a Symbol> {
    let query = query.trim();

    // Check for kind prefix: "fn foo", "struct Bar", "impl Baz"
    let (kind_filter, name_query) = parse_symbol_query(query);

    let mut results: Vec<&Symbol> = symbols
        .iter()
        .filter(|s| {
            // Apply kind filter if specified
            if let Some(kind) = kind_filter {
                if s.kind != kind {
                    return false;
                }
            }

            // Match against symbol name
            match &s.name {
                Some(name) => {
                    // Exact match scores highest, then case-insensitive, then substring
                    name == name_query
                        || name.eq_ignore_ascii_case(name_query)
                        || name.to_ascii_lowercase().contains(&name_query.to_ascii_lowercase())
                }
                None => false,
            }
        })
        .collect();

    // Sort: exact match first, then case-insensitive exact, then substring
    results.sort_by(|a, b| {
        let a_name = a.name.as_deref().unwrap_or("");
        let b_name = b.name.as_deref().unwrap_or("");
        let score = |n: &str| -> u8 {
            if n == name_query {
                0
            } else if n.eq_ignore_ascii_case(name_query) {
                1
            } else {
                2
            }
        };
        score(a_name).cmp(&score(b_name))
    });

    results
}

/// Parse a query like "fn foo" into (Some(Function), "foo") or (None, "foo").
fn parse_symbol_query(query: &str) -> (Option<SymbolKind>, &str) {
    let prefixes: &[(&str, SymbolKind)] = &[
        ("fn ", SymbolKind::Function),
        ("struct ", SymbolKind::Struct),
        ("enum ", SymbolKind::Enum),
        ("trait ", SymbolKind::Trait),
        ("impl ", SymbolKind::Impl),
        ("mod ", SymbolKind::Module),
        ("const ", SymbolKind::Const),
        ("static ", SymbolKind::Static),
        ("type ", SymbolKind::TypeAlias),
        ("macro ", SymbolKind::Macro),
    ];

    for (prefix, kind) in prefixes {
        if let Some(rest) = query.strip_prefix(prefix) {
            return (Some(*kind), rest.trim());
        }
    }

    (None, query)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RUST: &str = r#"
struct Foo {
    x: i32,
}

enum Bar {
    A,
    B(String),
}

trait Greetable {
    fn greet(&self) -> String;
}

impl Foo {
    fn new(x: i32) -> Self {
        Self { x }
    }

    fn value(&self) -> i32 {
        self.x
    }
}

impl Greetable for Foo {
    fn greet(&self) -> String {
        format!("Hello, I'm Foo({})", self.x)
    }
}

fn standalone() {
    println!("I'm a free function");
}

const MAX_SIZE: usize = 1024;
"#;

    #[test]
    fn extracts_top_level_symbols() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let names: Vec<_> = symbols
            .iter()
            .filter(|s| s.parent.is_none())
            .filter_map(|s| s.name.as_deref())
            .collect();

        assert!(names.contains(&"Foo"), "missing struct Foo: {names:?}");
        assert!(names.contains(&"Bar"), "missing enum Bar: {names:?}");
        assert!(names.contains(&"Greetable"), "missing trait Greetable: {names:?}");
        assert!(names.contains(&"standalone"), "missing fn standalone: {names:?}");
        assert!(names.contains(&"MAX_SIZE"), "missing const MAX_SIZE: {names:?}");
    }

    #[test]
    fn extracts_methods_with_parent() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let methods: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.parent.is_some())
            .collect();

        let new_fn = methods.iter().find(|s| s.name.as_deref() == Some("new"));
        assert!(new_fn.is_some(), "missing Foo::new");
        assert_eq!(new_fn.unwrap().parent.as_deref(), Some("Foo"));

        let greet_fn = methods
            .iter()
            .find(|s| s.name.as_deref() == Some("greet") && s.parent.as_deref() == Some("Greetable for Foo"));
        assert!(greet_fn.is_some(), "missing Greetable for Foo::greet");
    }

    #[test]
    fn find_by_exact_name() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let results = find_symbols(&symbols, "standalone");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, SymbolKind::Function);
    }

    #[test]
    fn find_by_kind_and_name() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let results = find_symbols(&symbols, "fn new");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].parent.as_deref(), Some("Foo"));
    }

    #[test]
    fn find_by_substring() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let results = find_symbols(&symbols, "stand");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name.as_deref(), Some("standalone"));
    }

    #[test]
    fn find_impl_block() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let results = find_symbols(&symbols, "impl Foo");
        // Matches both `impl Foo` and `impl Greetable for Foo`
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|s| s.kind == SymbolKind::Impl));
        // Exact match sorts first
        assert_eq!(results[0].name.as_deref(), Some("Foo"));
    }

    #[test]
    fn symbol_text_extraction() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let standalone = symbols
            .iter()
            .find(|s| s.name.as_deref() == Some("standalone"))
            .unwrap();
        let text = standalone.text(SAMPLE_RUST);
        assert!(text.contains("println!"), "function body not captured: {text}");
    }

    #[test]
    fn line_numbers_are_1_indexed() {
        let symbols = extract_rust_symbols(SAMPLE_RUST).unwrap();
        let foo = symbols
            .iter()
            .find(|s| s.name.as_deref() == Some("Foo") && s.kind == SymbolKind::Struct)
            .unwrap();
        assert!(foo.start_line >= 1);
        assert!(foo.end_line >= foo.start_line);
    }
}
