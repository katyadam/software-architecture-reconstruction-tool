use models::Field;
use tree_sitter::Node;

pub fn parse_superclasses(superclasses_node: Node, code: &str) -> Vec<String> {
    let cursor = &mut superclasses_node.walk();
    superclasses_node
        .named_children(cursor)
        .filter_map(|param| param.utf8_text(code.as_bytes()).ok().map(|s| s.to_string()))
        .collect()
}

pub fn parse_fields(fields_string: &str) -> Vec<Field> {
    let working_str = statix::strings::strip_outer_delimiters(fields_string, '(', ')');
    // For each parameter, extract its Field and collect only those that are Some()
    working_str
        .split([',', '\n'])
        .map(|s| s.trim_start())
        .filter(|s| *s != "self")
        .filter_map(parse_field)
        .collect()
}

/// Parses a single field string into a [`Field`].
///
/// Splits on the FIRST `=` only, so values containing `:` or `=`
/// (e.g. `"http://host:8000"`) are preserved intact.
/// Handles: `name`, `name: type`, `name = value`, `name: type = value`.
pub fn parse_field(field_string: &str) -> Option<Field> {
    let s = field_string.trim();
    if s.is_empty() || s.starts_with('(') {
        return None;
    }

    let (annotation, initial_value) = split_once_char(s, '=');
    let (name, datatype) = split_once_char(annotation.trim(), ':');

    let name = name.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let datatype = datatype.map(|d| d.trim().to_string());
    let is_collection = datatype.as_deref().is_some_and(is_collection_type);

    Some(Field {
        name,
        datatype,
        initial_value: initial_value.map(|v| v.trim().to_string()),
        datatype_signature: None,
        is_collection,
    })
}

/// Split `s` on the first occurrence of `sep`, returning `(before, Some(after))`.
/// Returns `(s, None)` when `sep` is absent.
fn split_once_char(s: &str, sep: char) -> (&str, Option<&str>) {
    match s.find(sep) {
        Some(pos) => (&s[..pos], Some(&s[pos + 1..])),
        None => (s, None),
    }
}

const PYTHON_COLLECTION_KEYWORDS: &[&str] = &[
    "list", "dict", "set", "tuple", "deque", "sequence", "mapping", "iterable", "List", "Dict",
    "Set", "Tuple", "Deque", "Sequence", "Mapping", "Iterable",
];

/// Returns `true` if `datatype` represents a Python collection type.
///
/// Matches both the bare name (`list`, `List`) and the generic form (`list[int]`,
/// `Dict[str, int]`). Both lowercase (`list`, `dict`) and capitalised (`List`, `Dict`)
/// variants are recognised.
pub fn is_collection_type(datatype: &str) -> bool {
    statix::strings::is_collection_type(datatype, PYTHON_COLLECTION_KEYWORDS, '[')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(
        name: &str,
        datatype: Option<&str>,
        initial_value: Option<&str>,
        is_collection: bool,
    ) -> Field {
        Field {
            name: name.to_string(),
            datatype: datatype.map(str::to_string),
            initial_value: initial_value.map(str::to_string),
            datatype_signature: None,
            is_collection,
        }
    }

    // ── bare name ──────────────────────────────────────────────────────────

    #[test]
    fn bare_name() {
        assert_eq!(parse_field("x"), Some(field("x", None, None, false)));
    }

    #[test]
    fn bare_name_whitespace() {
        assert_eq!(parse_field("  x  "), Some(field("x", None, None, false)));
    }

    // ── name with type annotation ──────────────────────────────────────────

    #[test]
    fn typed() {
        assert_eq!(
            parse_field("x: int"),
            Some(field("x", Some("int"), None, false))
        );
    }

    #[test]
    fn typed_extra_spaces() {
        assert_eq!(
            parse_field("  x :  str  "),
            Some(field("x", Some("str"), None, false))
        );
    }

    // ── name with default value ────────────────────────────────────────────

    #[test]
    fn default_only() {
        assert_eq!(
            parse_field("x = 42"),
            Some(field("x", None, Some("42"), false))
        );
    }

    #[test]
    fn default_none() {
        assert_eq!(
            parse_field("x = None"),
            Some(field("x", None, Some("None"), false))
        );
    }

    // ── typed with default ─────────────────────────────────────────────────

    #[test]
    fn typed_with_default() {
        assert_eq!(
            parse_field("x: int = 0"),
            Some(field("x", Some("int"), Some("0"), false))
        );
    }

    #[test]
    fn typed_with_string_default() {
        assert_eq!(
            parse_field(r#"x: str = "hello""#),
            Some(field("x", Some("str"), Some(r#""hello""#), false))
        );
    }

    // ── URL value preserves colon ──────────────────────────────────────────

    #[test]
    fn url_value_preserves_colon() {
        assert_eq!(
            parse_field(r#"cds_url: str = "http://host:8000""#),
            Some(field(
                "cds_url",
                Some("str"),
                Some(r#""http://host:8000""#),
                false
            ))
        );
    }

    #[test]
    fn url_no_type_preserves_colon() {
        assert_eq!(
            parse_field(r#"cds_url = "http://host:8000""#),
            Some(field("cds_url", None, Some(r#""http://host:8000""#), false))
        );
    }

    // ── value containing `=` preserves everything after the first `=` ─────

    #[test]
    fn value_with_equals_sign() {
        assert_eq!(
            parse_field("x = a=b"),
            Some(field("x", None, Some("a=b"), false))
        );
    }

    // ── collection types ───────────────────────────────────────────────────

    #[test]
    fn list_type_is_collection() {
        assert_eq!(
            parse_field("items: List[str]"),
            Some(field("items", Some("List[str]"), None, true))
        );
    }

    #[test]
    fn dict_type_is_collection() {
        assert_eq!(
            parse_field("mapping: Dict[str, int]"),
            Some(field("mapping", Some("Dict[str, int]"), None, true))
        );
    }

    // ── returns None ───────────────────────────────────────────────────────

    #[test]
    fn empty_string_returns_none() {
        assert_eq!(parse_field(""), None);
    }

    #[test]
    fn whitespace_only_returns_none() {
        assert_eq!(parse_field("   "), None);
    }

    #[test]
    fn starts_with_paren_returns_none() {
        assert_eq!(parse_field("(self, id: int, name: str = \"x\")"), None);
    }

    #[test]
    fn colon_only_no_name_returns_none() {
        assert_eq!(parse_field(": int"), None);
    }
}
