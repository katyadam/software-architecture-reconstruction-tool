//! Call-site lookups shared by DDFA and the agentic tools.

use std::path::Path;

use log::warn;
use models::{Argument, ConfigurationData, ParsedCallStatement, source_code::SourceSpan};

/// `CallStatement` `rc` was identified from: same file, enclosing function and arguments.
// ponytail: same function + same args on two receivers -> first wins; add a call-site span to RestCall if it bites.
pub(crate) fn source_call<'a>(
    function_hash: &str,
    args: &[Argument],
    calls: &'a [ParsedCallStatement],
) -> Option<&'a ParsedCallStatement> {
    calls.iter().find(|c| {
        c.metadata
            .enclosing_function_hash
            .as_deref()
            .unwrap_or_default()
            == function_hash
            && c.metadata.arguments == args
    })
}

/// Source text of `span`, read from disk on demand; `None` if unreadable.
pub(crate) fn read_span(file_path: &str, span: &SourceSpan) -> Option<String> {
    match std::fs::read(file_path) {
        Ok(bytes) => slice_span(&bytes, span),
        Err(e) => {
            warn!("jev: cannot read {file_path} for snippet: {e}");
            None
        }
    }
}

/// `None` if `span` is out of range or not UTF-8.
fn slice_span(bytes: &[u8], span: &SourceSpan) -> Option<String> {
    let text = bytes.get(span.start_byte as usize..span.end_byte as usize)?;
    std::str::from_utf8(text).ok().map(str::to_string)
}

/// Java `function_name` carries args: `rt.exchange(url, x)` -> `rt.exchange`.
/// Python/Go names don't end in `)`, so `requests.Session().get` stays intact.
// ponytail: paren counting ignores string literals; a paren inside a literal arg mis-splits.
pub fn strip_args(function_name: &str) -> &str {
    if !function_name.ends_with(')') {
        return function_name;
    }
    let mut depth = 0;
    for (i, c) in function_name.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return &function_name[..i];
                }
            }
            _ => {}
        }
    }
    function_name
}

/// From service base dir on; file name only otherwise. Never the absolute path.
pub fn relative_file(file_path: &str, config: &ConfigurationData) -> String {
    config
        .service_descriptions
        .iter()
        .find_map(|s| {
            file_path
                .find(&s.base_dir_path)
                .map(|i| file_path[i..].to_string())
        })
        .or_else(|| {
            Path::new(file_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_span_cuts_function_and_rejects_bad_spans() {
        let code = "import x\n\ndef f(s):\n    return s.get(u)\n";
        let start = code.find("def").expect("def") as u32;
        let end = code.trim_end().len() as u32;
        assert_eq!(
            slice_span(code.as_bytes(), &SourceSpan::new(start, end)).as_deref(),
            Some("def f(s):\n    return s.get(u)")
        );
        assert_eq!(slice_span(code.as_bytes(), &SourceSpan::new(0, 999)), None);
        assert_eq!(slice_span("é".as_bytes(), &SourceSpan::new(0, 1)), None);
    }

    #[test]
    fn strip_args_only_strips_trailing_call() {
        assert_eq!(strip_args("rt.exchange(url, a(b))"), "rt.exchange");
        assert_eq!(strip_args("b.uri(x).get(y)"), "b.uri(x).get");
        assert_eq!(
            strip_args("requests.Session().get"),
            "requests.Session().get"
        );
    }
}
