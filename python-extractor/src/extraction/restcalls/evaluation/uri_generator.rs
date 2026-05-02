use std::collections::HashMap;

use models::ir::ast::Expr;
use statix::symbolic::AnalysisResult;

type ResolvedPart = Vec<String>;

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
/// 3 possible values respectively produce 6 URIs.
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
            let mut next_acc = Vec::with_capacity(acc.len() * parts.len());
            for base in &acc {
                for part in &parts {
                    let mut new_uri = String::with_capacity(base.len() + part.len());
                    new_uri.push_str(base);
                    new_uri.push_str(part);
                    next_acc.push(new_uri);
                }
            }
            next_acc
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

        if part.starts_with('"') && part.ends_with('"') {
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
