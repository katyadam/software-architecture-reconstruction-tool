use std::collections::HashMap;

use models::ir::ast::Expr;
use statix::symbolic::AnalysisResult;

type ResolvedPart = Vec<String>;

/// Upper bound on URIs generated for one call target (see #57).
const MAX_TARGET_URIS: usize = 256;

/// Resolves a URI template into every possible concrete URI.
///
/// The template is a Python string-concatenation expression (parts separated by `+`).
/// Each part is resolved against the symbolic `analysis_result` environment:
/// - Quoted string literals have their quotes stripped.
/// - Variables mapped to a single `Expr::Literal` produce one value.
/// - Variables mapped to `Expr::Joined` expand the result set.
/// - f-string literals are resolved by [`resolve_fstring`].
///
/// The resolved parts are combined via Cartesian product, so two parts with 2 and
/// 3 possible values respectively produce 6 URIs. At most `MAX_TARGET_URIS` (256) are
/// returned: the first ones in that combination order.
pub fn generate_target_uris(
    template: &str,
    analysis_result: &AnalysisResult,
    enums_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let resolved_parts: Vec<ResolvedPart> =
        get_resolved_parts(template, analysis_result, enums_map);

    resolved_parts
        .into_iter()
        .fold(vec!["".to_string()], |acc, parts| {
            acc.iter()
                .flat_map(|base| parts.iter().map(move |part| format!("{base}{part}")))
                .take(MAX_TARGET_URIS)
                .collect()
        })
}

fn get_resolved_parts(
    template: &str,
    analysis_result: &AnalysisResult,
    enums_map: &HashMap<String, Vec<String>>,
) -> Vec<ResolvedPart> {
    let mut all_parts = Vec::new();

    for part in template.split('+') {
        let part = part.trim();
        let lookup_key = part.trim_end_matches(".value");
        let expr = analysis_result.final_env.get(lookup_key);

        if part.len() >= 2 && part.starts_with('"') && part.ends_with('"') {
            all_parts.push(vec![part[1..part.len() - 1].to_string()]);
            continue;
        }

        match expr {
            Some((_, Expr::Literal(s))) => {
                if s.contains('{') || s.starts_with('f') {
                    all_parts.extend(resolve_fstring(s, analysis_result, enums_map));
                } else {
                    all_parts.push(vec![s.clone()]);
                }
            }
            Some((_, Expr::Joined { vals })) => {
                let flattened: Vec<String> = vals
                    .iter()
                    .filter_map(|val| {
                        if let Expr::Literal(s) = val {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                all_parts.push(flattened);
            }

            // Used when the target URL is directly an f-string
            _ if part.contains('{') => {
                all_parts.extend(resolve_fstring(part, analysis_result, enums_map));
            }
            _ => all_parts.push(vec![part.to_string()]),
        }
    }
    all_parts
}

/// Parses a Python f-string character-by-character and resolves it into a list
/// of `ResolvedPart`s (one per segment).
///
/// Literal text segments become single-element `Vec`s. `{var}` placeholders are
/// resolved against `analysis_result`:
/// - `Expr::Literal` -> single value
/// - `Expr::Joined` -> multiple values (expands the Cartesian product)
/// - `Expr::Var` with a known enum dtype -> all enum variants from `enums_map`
/// - Anything else -> the placeholder text unchanged (`{var}`)
///
/// Escaped braces (`{{` / `}}`) are treated as literal `{` / `}`.
fn resolve_fstring(
    string_literal: &str,
    analysis_result: &AnalysisResult,
    enums_map: &HashMap<String, Vec<String>>,
) -> Vec<ResolvedPart> {
    let mut resolved_parts = Vec::new();
    let mut current_literal = String::new();

    let content = string_literal
        .strip_prefix('f')
        .unwrap_or(string_literal)
        .trim_matches(|c| c == '"' || c == '\'');

    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    current_literal.push(chars.next().unwrap());
                    continue;
                }
                if !current_literal.is_empty() {
                    resolved_parts.push(vec![current_literal.clone()]);
                    current_literal.clear();
                }

                let mut expr_str = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '}' {
                        chars.next();
                        break;
                    }
                    expr_str.push(chars.next().unwrap());
                }

                let var_name = expr_str.trim().trim_end_matches(".value");
                let resolved = match analysis_result.final_env.get(var_name) {
                    Some((_, Expr::Literal(s))) => vec![s.clone()],
                    Some((_, Expr::Joined { vals })) => vals
                        .iter()
                        .filter_map(|val| {
                            if let Expr::Literal(s) = val {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                    Some((dtype, Expr::Var(_))) => dtype
                        .as_deref()
                        .and_then(|t| enums_map.get(t))
                        .cloned()
                        .unwrap_or_else(|| vec![format!("{{{}}}", var_name)]),
                    _ => vec![format!("{{{}}}", var_name)],
                };
                resolved_parts.push(resolved);
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    current_literal.push(chars.next().unwrap());
                } else {
                    current_literal.push(c);
                }
            }
            _ => current_literal.push(c),
        }
    }

    if !current_literal.is_empty() {
        resolved_parts.push(vec![current_literal]);
    }
    resolved_parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_result() -> AnalysisResult {
        AnalysisResult {
            return_value: Expr::Literal(String::new()),
            final_env: HashMap::new(),
        }
    }

    #[test]
    fn lone_quote_part_does_not_panic() {
        // `"a+" + x` splits on `+` into `"a`, `"` and `x`; the lone `"` used to panic.
        let uris = generate_target_uris("\"a+\" + x", &empty_result(), &HashMap::new());
        assert_eq!(uris, vec!["\"a\"x".to_string()]);
    }

    #[test]
    fn empty_literal_part_resolves_to_empty_string() {
        let uris = generate_target_uris("\"\" + \"/api\"", &empty_result(), &HashMap::new());
        assert_eq!(uris, vec!["/api".to_string()]);
    }

    fn joined(env: &mut AnalysisResult, name: &str, vals: &[&str]) {
        let vals = vals.iter().map(|v| Expr::Literal(v.to_string())).collect();
        env.final_env
            .insert(name.to_string(), (None, Expr::Joined { vals }));
    }

    #[test]
    fn expansion_is_capped_deterministically() {
        let mut env = empty_result();
        let names: Vec<String> = (0..20).map(|i| format!("v{i}")).collect();
        for name in &names {
            joined(&mut env, name, &["a", "b"]);
        }
        let uris = generate_target_uris(&names.join(" + "), &env, &HashMap::new());
        assert_eq!(uris.len(), MAX_TARGET_URIS);
        assert_eq!(uris[0], "a".repeat(20));
        assert_eq!(uris[1], "a".repeat(19) + "b");
        assert_eq!(uris[255], "a".repeat(12) + &"b".repeat(8));
    }

    #[test]
    fn small_expansion_is_unchanged() {
        let mut env = empty_result();
        joined(&mut env, "x", &["a", "b"]);
        joined(&mut env, "y", &["1", "2", "3"]);
        let uris = generate_target_uris("x + y", &env, &HashMap::new());
        assert_eq!(uris, vec!["a1", "a2", "a3", "b1", "b2", "b3"]);
    }
}
