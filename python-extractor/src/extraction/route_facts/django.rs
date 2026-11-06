use models::{Argument, RouteFact, Scope, ir::project::TypedFileRecord};
use statix::strings::strip_quotes;

use crate::extraction::endpoints::normalize_python_route;

const URLPATTERNS: &str = "urlpatterns";
const REGEX_METACHARACTERS: &[char] = &[
    '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\',
];

/// Django: module-level `path(route, include("a.b"))`, `re_path(regex, include("a.b"))`.
pub(super) fn facts(file: &TypedFileRecord) -> Vec<RouteFact> {
    file.call_statements
        .iter()
        .map(|c| &c.metadata)
        .filter(|c| c.enclosing_function_name.is_none())
        .filter_map(|c| match c.function_name.as_str() {
            "path" => include_fact(&c.arguments, path_prefix),
            "re_path" => include_fact(&c.arguments, regex_prefix),
            _ => None,
        })
        .collect()
}

/// Arguments arrive naively split on `,` and `=`, so `include(("a.b", "app"), namespace="x")`
/// is the fragments `include(("a.b"`, `"app")`, `namespace` = `"x"))`.
// ponytail: a prefix holding `,` shifts the fragments -> no fact, today's URI; needs a syntax-aware split.
fn include_fact(arguments: &[Argument], prefix: fn(&str) -> Option<String>) -> Option<RouteFact> {
    let positional = |index: usize| {
        let argument = arguments.get(index)?;
        argument
            .assigned_variable
            .is_empty()
            .then(|| argument.value.trim())
    };
    let module = included_module(positional(1)?)?;
    let path = module.replace('.', "/");
    Some(RouteFact::AttachModule {
        scope: Scope::Global,
        parent: URLPATTERNS.to_string(),
        paths: vec![format!("{path}.py"), format!("{path}/__init__.py")],
        child: URLPATTERNS.to_string(),
        prefix: prefix(positional(0)?)?,
    })
}

/// `a.b` of the first fragment of an `include("a.b")` or `include(("a.b", ...))` call.
fn included_module(fragment: &str) -> Option<String> {
    let inner = fragment
        .strip_prefix("include(")?
        .trim_start_matches(|c: char| c == '(' || c.is_whitespace())
        .trim_end_matches(|c: char| c == ')' || c.is_whitespace());
    literal(inner).filter(|module| module.split('.').all(is_identifier))
}

fn is_identifier(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with(|c: char| c.is_ascii_digit())
        && part.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Content of a quoted or `r`-quoted string literal.
fn literal(text: &str) -> Option<String> {
    let text = text.strip_prefix(['r', 'R']).unwrap_or(text);
    let inner = strip_quotes(text);
    (inner.len() < text.len()).then_some(inner)
}

/// Literal route with converters normalized; anything else is raw text for pass 3.
fn path_prefix(route: &str) -> Option<String> {
    Some(match literal(route) {
        Some(route) => format!("\"{}\"", normalize_python_route(&route)),
        None => route.to_string(),
    })
}

/// Literal regex that is a plain path once its leading `^` is gone.
fn regex_prefix(regex: &str) -> Option<String> {
    let regex = literal(regex)?;
    let plain = regex.strip_prefix('^').unwrap_or(&regex);
    (!plain.contains(REGEX_METACHARACTERS)).then(|| format!("\"{plain}\""))
}

#[cfg(test)]
mod tests {
    use models::{RouteFact, Scope};

    use super::facts;
    use crate::extraction::parse::extract_syntactic;

    fn facts_of(code: &str) -> Vec<RouteFact> {
        let record: models::ir::project::TypedFileRecord =
            extract_syntactic(code, "svc/urls.py").unwrap().into();
        facts(&record)
    }

    fn app_urls(prefix: &str) -> RouteFact {
        RouteFact::AttachModule {
            scope: Scope::Global,
            parent: "urlpatterns".to_string(),
            paths: vec![
                "app/urls.py".to_string(),
                "app/urls/__init__.py".to_string(),
            ],
            child: "urlpatterns".to_string(),
            prefix: prefix.to_string(),
        }
    }

    /// `call` as the only `urlpatterns` entry.
    fn entry(call: &str) -> Vec<RouteFact> {
        facts_of(&format!("urlpatterns = [\n    {call},\n]\n"))
    }

    #[test]
    fn module_string() {
        assert_eq!(
            entry(r#"path("api/", include("app.urls"))"#),
            vec![app_urls(r#""api/""#)]
        );
    }

    #[test]
    fn single_quotes_and_multiline() {
        assert_eq!(
            entry("path(\n  'api/',\n  include( 'app.urls' ),\n)"),
            vec![app_urls(r#""api/""#)]
        );
    }

    #[test]
    fn tuple_namespace_and_name_fragments() {
        for call in [
            r#"path("a/", include(("app.urls", "app")))"#,
            r#"path("a/", include("app.urls", namespace="x"))"#,
            r#"path("a/", include(("app.urls", "app"), namespace="x"), name="y")"#,
        ] {
            assert_eq!(entry(call), vec![app_urls(r#""a/""#)], "{call}");
        }
    }

    #[test]
    fn route_literal_normalized_and_requoted() {
        assert_eq!(
            entry(r#"path(r'<int:org>/', include("app.urls"))"#),
            vec![app_urls(r#""{org}/""#)]
        );
    }

    #[test]
    fn non_literal_route_passes_raw() {
        assert_eq!(
            entry(r#"path(API, include("app.urls"))"#),
            vec![app_urls("API")]
        );
    }

    #[test]
    fn route_holding_equals_or_comma_gives_no_fact() {
        assert_eq!(entry(r#"path("id=<x>/", include("app.urls"))"#), vec![]);
        assert_eq!(entry(r#"path("a,b/", include("app.urls"))"#), vec![]);
    }

    #[test]
    fn regex_literal_loses_caret() {
        assert_eq!(
            entry(r#"re_path(r"^api/", include("app.urls"))"#),
            vec![app_urls(r#""api/""#)]
        );
    }

    #[test]
    fn regex_with_metacharacters_gives_no_fact() {
        for call in [
            r#"re_path(r"^v(?P<n>\d+)/", include("app.urls"))"#,
            r#"re_path(r"^api/$", include("app.urls"))"#,
            r#"re_path(r"^v1.0/", include("app.urls"))"#,
        ] {
            assert_eq!(entry(call), vec![], "{call}");
        }
    }

    #[test]
    fn regex_constant_gives_no_fact() {
        assert_eq!(entry(r#"re_path(API, include("app.urls"))"#), vec![]);
    }

    #[test]
    fn non_literal_or_invalid_module_gives_no_fact() {
        for call in [
            r#"path("a/", include(router.urls))"#,
            r#"path("a/", include([path("b/", v)]))"#,
            r#"path("a/", include("../secret"))"#,
            r#"path("a/", include(".urls"))"#,
            r#"path("a/", include("app..urls"))"#,
            r#"path("a/", include(f"{x}.urls"))"#,
        ] {
            assert_eq!(entry(call), vec![], "{call}");
        }
    }

    #[test]
    fn plain_view_gives_no_fact() {
        assert_eq!(entry(r#"path("a/", views.a)"#), vec![]);
    }

    #[test]
    fn call_inside_function_gives_no_fact() {
        let code = "def f():\n    return path(\"a/\", include(\"app.urls\"))\n";
        assert_eq!(facts_of(code), vec![]);
    }
}
