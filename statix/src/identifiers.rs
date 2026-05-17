//! Tree-sitter-backed identifier extraction for code snippets.
//!
//! Walks the parsed syntax tree and collects only identifier-shaped leaves
//! (named children whose `kind()` matches one of the per-language identifier
//! kinds). String literals, comments, numeric literals, and punctuation are
//! skipped — which is the main reason this exists rather than a regex pass
//! over the raw source.

use std::collections::HashSet;

use models::ir::language::Language;
use tree_sitter::Node;

/// Parse `code` with the tree-sitter grammar for `language` and return the
/// set of identifier texts that appear in the tree. Identifiers inside
/// string literals or comments are not included because those leaves are
/// not identifier-kind nodes in the grammar.
///
/// Returns an empty set if the grammar fails to load or the source fails to
/// parse — callers treat the absence of identifiers as "no signal" rather
/// than an error.
pub fn identifiers_in_snippet(code: &str, language: Language) -> HashSet<String> {
    let (ts_lang, kinds): (tree_sitter::Language, &[&str]) = match language {
        Language::Java => (crate::java::ts_language(), crate::java::IDENTIFIER_KINDS),
        Language::Python => (
            crate::python::ts_language(),
            crate::python::IDENTIFIER_KINDS,
        ),
        Language::Unknown => return HashSet::new(),
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return HashSet::new();
    }
    let Some(tree) = parser.parse(code, None) else {
        return HashSet::new();
    };
    let mut out = HashSet::new();
    collect(tree.root_node(), code.as_bytes(), kinds, &mut out);
    out
}

fn collect(node: Node, src: &[u8], kinds: &[&str], out: &mut HashSet<String>) {
    if kinds.contains(&node.kind())
        && let Ok(t) = node.utf8_text(src)
    {
        out.insert(t.to_string());
    }
    let mut c = node.walk();
    for child in node.named_children(&mut c) {
        collect(child, src, kinds, out);
    }
}

/// Why is there also print as an identifier, when these identifiers
/// are mainly used to prune the input variables set to LLM call?
/// Answer is that it is easier to over-aproximate all identifiers,
/// and then filter them (print will definitely be filtered), than to
/// waste time with walking up to parents, to ask, whether the identifier is
/// call or variable.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_identifiers_skips_string_literals() {
        let code = "x = MDS_URL\nprint(\"BASE_URL\")";
        let set = identifiers_in_snippet(code, Language::Python);
        assert!(set.contains("MDS_URL"), "expected MDS_URL, got {set:?}");
        assert!(set.contains("print"), "expected print, got {set:?}");
        assert!(
            !set.contains("BASE_URL"),
            "BASE_URL is inside a string literal and must not be collected, got {set:?}",
        );
    }

    #[test]
    fn java_identifiers_includes_type_names() {
        let code = "class C { void m() { String url = MyConstants.BASE; } }";
        let set = identifiers_in_snippet(code, Language::Java);
        for expected in ["String", "url", "MyConstants", "BASE"] {
            assert!(set.contains(expected), "expected {expected} in {set:?}",);
        }
    }
}
