/// Strips outer quotes from a string.
///
/// Handles triple-quotes (`"""`, `'''`), single-quotes (`'`), and double-quotes (`"`).
/// Returns the input unchanged if no matching quotes are found.
pub fn strip_quotes(s: &str) -> String {
    if s.starts_with("\"\"\"") && s.ends_with("\"\"\"") && s.len() >= 6 {
        return s[3..s.len() - 3].to_string();
    }
    if s.starts_with("'''") && s.ends_with("'''") && s.len() >= 6 {
        return s[3..s.len() - 3].to_string();
    }
    if (s.starts_with('"') && s.ends_with('"') || s.starts_with('\'') && s.ends_with('\''))
        && s.len() >= 2
    {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Strips Python string literal prefixes (`f`, `b`, `r`, `fr`, `rf`, `br`, `rb`)
/// and outer quotes from a string.
pub fn clean_python_string(s: &str) -> String {
    let s = s.trim();

    if let Some(quote_start) = s.find(['"', '\'']) {
        let content = &s[quote_start..];
        return strip_quotes(content);
    }

    s.to_string()
}

/// Splits `input` at `delimiter` characters, but only when not nested inside
/// any of the given bracket pairs.
///
/// `delimiters` lists the characters to split on (e.g. `&[',']` or `&[',', '\n']`).
/// `brackets` lists `(open, close)` pairs to track depth for (e.g. `&[('<', '>')]`
/// or `&[('(', ')'), ('[', ']'), ('{', '}')]`).
///
/// Empty segments (after trimming) are excluded from the result.
pub fn split_at_top_level(
    input: &str,
    delimiters: &[char],
    brackets: &[(char, char)],
) -> Vec<String> {
    let mut result = Vec::new();
    let mut depths: Vec<i32> = vec![0; brackets.len()];
    let mut current = String::new();

    for c in input.chars() {
        let mut is_open = false;
        let mut is_close = false;

        for (i, &(open, close)) in brackets.iter().enumerate() {
            if c == open {
                depths[i] += 1;
                is_open = true;
                break;
            } else if c == close {
                depths[i] = (depths[i] - 1).max(0);
                is_close = true;
                break;
            }
        }

        let at_top_level = depths.iter().all(|&d| d == 0);

        if delimiters.contains(&c) && at_top_level && !is_open && !is_close {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
            current.clear();
        } else {
            current.push(c);
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        result.push(trimmed.to_string());
    }

    result
}

/// Recursively unwraps a generic type to find the innermost element type.
///
/// Given `open`/`close` bracket characters, extracts the content between
/// the outermost brackets and recurses. For multi-argument generics
/// (e.g. `Dict[str, User]`), the last type argument is used.
///
/// Examples with `[`/`]`: `list[User]` -> `User`, `Dict[str, User]` -> `User`
/// Examples with `<`/`>`: `List<User>` -> `User`, `Map<String, User>` -> `User`
pub fn extract_inner_generic_type(datatype: &str, open: char, close: char) -> &str {
    let d = datatype.trim();

    if let Some(start_index) = d.find(open)
        && let Some(end_index) = d.rfind(close)
    {
        let inner = d[start_index + 1..end_index].trim();

        if inner.contains(',') {
            let parts: Vec<&str> = inner.split(',').collect();
            let last_part = parts.last().unwrap_or(&inner).trim();
            return extract_inner_generic_type(last_part, open, close);
        }

        return extract_inner_generic_type(inner, open, close);
    }

    d
}

/// Returns `true` if `datatype` matches one of the `keywords` as a collection type.
///
/// Matches both bare names (`list`, `List`) and generic forms (`list[int]`, `List<String>`)
/// by extracting the base type before `open_bracket` and comparing against keywords.
/// Both exact matches and fully-qualified suffixes (e.g. `java.util.List`) are accepted.
pub fn is_collection_type(datatype: &str, keywords: &[&str], open_bracket: char) -> bool {
    let d = datatype.trim();
    if d.is_empty() {
        return false;
    }

    let base_type = d.split(open_bracket).next().unwrap_or("").trim();

    keywords
        .iter()
        .any(|&k| base_type == k || base_type.ends_with(&format!(".{}", k)))
}

/// Collapses runs of whitespace (including newlines) into single spaces and trims.
pub fn normalize_whitespace(input: &str) -> String {
    input
        .replace(['\n', '\r'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strips matching outer delimiters from a string.
///
/// If `s` starts with `open` and ends with `close`, returns the inner content.
/// Otherwise returns `s` unchanged (after trimming).
pub fn strip_outer_delimiters(s: &str, open: char, close: char) -> &str {
    let s = s.trim();
    if s.starts_with(open) && s.ends_with(close) && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes(r#""hello""#), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes(r#""""hello""""#), "hello");
        assert_eq!(strip_quotes("'''hello'''"), "hello");
        assert_eq!(strip_quotes("no_quotes"), "no_quotes");
    }

    #[test]
    fn test_clean_python_string() {
        assert_eq!(clean_python_string(r#"f"hello""#), "hello");
        assert_eq!(clean_python_string(r#"b"bytes""#), "bytes");
        assert_eq!(clean_python_string(r#""plain""#), "plain");
        assert_eq!(clean_python_string("no_quotes"), "no_quotes");
    }

    #[test]
    fn test_split_at_top_level_java() {
        let result = split_at_top_level("Map<String, Integer>, List<User>", &[','], &[('<', '>')]);
        assert_eq!(result, vec!["Map<String, Integer>", "List<User>"]);
    }

    #[test]
    fn test_split_at_top_level_python() {
        let result = split_at_top_level(
            "a: int, b: list[str], c: dict[str, int]",
            &[','],
            &[('(', ')'), ('[', ']'), ('{', '}')],
        );
        assert_eq!(result, vec!["a: int", "b: list[str]", "c: dict[str, int]"]);
    }

    #[test]
    fn test_split_at_top_level_with_newlines() {
        let result = split_at_top_level(
            "a: int\nb: str\nc: float",
            &[',', '\n'],
            &[('(', ')'), ('[', ']'), ('{', '}')],
        );
        assert_eq!(result, vec!["a: int", "b: str", "c: float"]);
    }

    #[test]
    fn test_extract_inner_generic_type_python() {
        assert_eq!(extract_inner_generic_type("list[User]", '[', ']'), "User");
        assert_eq!(
            extract_inner_generic_type("Dict[str, User]", '[', ']'),
            "User"
        );
        assert_eq!(
            extract_inner_generic_type("Optional[list[User]]", '[', ']'),
            "User"
        );
        assert_eq!(extract_inner_generic_type("str", '[', ']'), "str");
    }

    #[test]
    fn test_extract_inner_generic_type_java() {
        assert_eq!(extract_inner_generic_type("List<User>", '<', '>'), "User");
        assert_eq!(
            extract_inner_generic_type("Map<String, User>", '<', '>'),
            "User"
        );
        assert_eq!(extract_inner_generic_type("String", '<', '>'), "String");
    }

    #[test]
    fn test_is_collection_type_python() {
        let keywords = &["list", "dict", "set", "List", "Dict", "Set"];
        assert!(is_collection_type("list[int]", keywords, '['));
        assert!(is_collection_type("Dict[str, int]", keywords, '['));
        assert!(is_collection_type("list", keywords, '['));
        assert!(!is_collection_type("str", keywords, '['));
    }

    #[test]
    fn test_is_collection_type_java() {
        let keywords = &["List", "ArrayList", "Map", "HashMap", "Set"];
        assert!(is_collection_type("List<String>", keywords, '<'));
        assert!(is_collection_type("HashMap<K, V>", keywords, '<'));
        assert!(is_collection_type("java.util.List<String>", keywords, '<'));
        assert!(!is_collection_type("String", keywords, '<'));
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("a\n  b\n  c"), "a b c");
    }

    #[test]
    fn test_strip_outer_delimiters() {
        assert_eq!(strip_outer_delimiters("(a, b)", '(', ')'), "a, b");
        assert_eq!(strip_outer_delimiters("no_parens", '(', ')'), "no_parens");
        assert_eq!(strip_outer_delimiters("[x]", '[', ']'), "x");
    }
}
