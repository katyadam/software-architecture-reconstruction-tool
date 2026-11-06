use std::str::FromStr;

use models::{Argument, CallStatement, HttpMethod, RestCall};

use crate::extraction::restcalls::identification::{
    HTTP_METHODS, strategy::IdentificationStrategy,
};

const QUOTES: [char; 2] = ['"', '\''];

#[derive(Default)]
pub struct MethodCallIdentificationStrategy {}

impl MethodCallIdentificationStrategy {
    pub fn new() -> Self {
        Self {}
    }

    fn identify_http_method(&self, function_name: &str) -> Option<HttpMethod> {
        if let Some(last) = function_name.split(".").last() {
            HTTP_METHODS
                .iter()
                .find(|m| last.to_ascii_lowercase() == **m)
                .and_then(|m| HttpMethod::from_str(m).ok())
        } else {
            None
        }
    }

    fn identify_target_uri(&self, call_args: &[Argument]) -> Option<String> {
        let raw = &call_args.first()?.value;
        if let Some(target) = concat_target(raw) {
            return Some(target);
        }
        let uri = statix::strings::clean_python_string(raw);
        let is_string_literal = raw.contains('"') || raw.contains('\'');
        // String literals with no `/` are not URIs (e.g. dict key lookups like `.get("key")`).
        // Bare variable names (no quotes) are passed through — evaluation resolves them later.
        if is_string_literal && !uri.contains('/') {
            return None;
        }
        Some(uri)
    }
}

/// Inline `BASE + "/x"` as the template pass 3 evaluates, names as `{BASE}` holes.
/// `None`: other form, take the plain path.
fn concat_target(raw: &str) -> Option<String> {
    // Pass 3 splits on every `+`, so a `+` in quoted text cannot survive it.
    let mut rest = raw;
    while let Some(start) = rest.find(QUOTES) {
        let (content, after) = take_quoted(&rest[start..])?;
        if content.contains('+') {
            return None;
        }
        rest = after;
    }

    // No bracket tracking: a `+` inside brackets leaves fragments that fail below.
    let operands: Vec<&str> = raw.split('+').map(str::trim).collect();
    if operands.len() < 2 || leading_literal(operands[0]).is_some() {
        return None;
    }

    let mut has_path = false;
    let template = operands
        .iter()
        .map(|operand| match leading_literal(operand) {
            Some((prefix, content, "")) => {
                has_path |= content.contains('/');
                let is_fstring = prefix.contains(['f', 'F']) && content.contains('{');
                Some(format!(
                    "{}\"{content}\"",
                    if is_fstring { "f" } else { "" }
                ))
            }
            Some(_) => None,
            None => is_dotted_name(operand).then(|| format!("{{{operand}}}")),
        })
        .collect::<Option<Vec<_>>>()?
        .join(" + ");
    has_path.then_some(template)
}

/// `BASE`, `self.base`, `a.b.c`.
fn is_dotted_name(text: &str) -> bool {
    text.split('.').all(|name| {
        let mut chars = name.chars();
        chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && chars.all(|c| c.is_alphanumeric() || c == '_')
    })
}

/// `(prefix, content, remainder)` when `operand` starts with a string literal.
fn leading_literal(operand: &str) -> Option<(&str, &str, &str)> {
    let (prefix, quoted) = operand.split_at(operand.find(QUOTES)?);
    if prefix.len() > 2 || !prefix.chars().all(|c| "rRbBfFuU".contains(c)) {
        return None;
    }
    let (content, after) = take_quoted(quoted)?;
    Some((prefix, content, after))
}

/// `(content, remainder)` of text starting at a quote. `None` when unterminated.
fn take_quoted(text: &str) -> Option<(&str, &str)> {
    let quote = ["\"\"\"", "'''", "\"", "'"]
        .into_iter()
        .find(|q| text.starts_with(q))?;
    let body = &text[quote.len()..];
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if body[i..].starts_with(quote) {
            return Some((&body[..i], &body[i + quote.len()..]));
        }
    }
    None
}

impl IdentificationStrategy for MethodCallIdentificationStrategy {
    // To recognize REST call we just need the function_name to end with any HTTP method
    fn identify_restcall(&self, call: &CallStatement, file_path: &str) -> Option<RestCall> {
        let http_method = self.identify_http_method(&call.function_name)?;
        let target_uri = self.identify_target_uri(&call.arguments)?;
        // FastAPI uses @app.<method> to declare an endpoint — not an outbound call.
        if call.is_decorator {
            return None;
        }
        if call.enclosing_function_name.is_none() && call.enclosing_class_name.is_none() {
            return None;
        }
        Some(RestCall {
            function_name: call.enclosing_function_name.clone().unwrap_or_default(),
            function_hash: call.enclosing_function_hash.clone().unwrap_or_default(),
            call_arguments: call.arguments.clone(),
            http_method,
            target_uri,
            file_path: file_path.to_string(),
            source_span: call.source_span.clone(),
        })
    }
}
