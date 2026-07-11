use models::ir::ast::Expr;
use statix::symbolic::AnalysisResult;

type ResolvedPart = Vec<String>;

/// Resolves a URI template string into every possible concrete URI.
///
/// The template is a Java string-concatenation expression (parts separated by `+`).
/// Each part is looked up in the symbolic `analysis_result` environment:
/// - String literals have their quotes stripped.
/// - Variables mapped to a single `Expr::Literal` produce one value.
/// - Variables mapped to `Expr::Joined` (multiple possible values) expand the result set.
///
/// The resolved parts are combined via Cartesian product, so a template with two
/// `Joined` parts of sizes 2 and 3 produces 6 URIs.
pub fn generate_target_uris(template: &str, analysis_result: &AnalysisResult) -> Vec<String> {
    let resolved_parts: Vec<ResolvedPart> = get_resolved_parts(template, analysis_result);

    // Fold generated parts into actual URIs
    // For example: [vec!["a"], vec!["ba", "ab"], vec!["c"]] ~~> vec!["abac", "aabc"]
    resolved_parts
        .into_iter()
        .fold(vec!["".to_string()], |acc, parts| {
            let mut next_acc = Vec::new();
            for base in &acc {
                for part in &parts {
                    next_acc.push(format!("{}{}", base, part));
                }
            }
            next_acc
        })
}

fn get_resolved_parts(template: &str, analysis_result: &AnalysisResult) -> Vec<ResolvedPart> {
    template
        .split('+')
        .map(|part| {
            let part = part.trim(); // Handle potential whitespace around '+'
            let expr = analysis_result.final_env.get(part);
            // If it's a literal quote "value", strip the quotes
            if part.starts_with('"') && part.ends_with('"') {
                return vec![part[1..part.len() - 1].to_string()];
            }
            // Otherwise, treat it as a variable and look it up in the environment
            match expr {
                Some((_, Expr::Literal(s))) => vec![s.clone()],
                // If there are Joined Literals, then map them into Vector of Strings
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
                _ => vec![part.to_string()],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use statix::symbolic::AnalysisResult;

    /// Mirrors the pass3 Err-branch path: no method env is available (default
    /// analysis), but a fully-literal URL must still resolve with its quotes
    /// stripped -- not survive as a quoted residual that fails the `http` gate.
    #[test]
    fn pure_literal_url_resolves_without_env() {
        let uris =
            generate_target_uris("\"http://ts-x-service:8000/api/v1\"", &AnalysisResult::default());
        assert_eq!(uris, vec!["http://ts-x-service:8000/api/v1".to_string()]);
    }

    /// Concatenated literals (a common Java pattern) also resolve env-free.
    #[test]
    fn concatenated_literals_resolve_without_env() {
        let uris = generate_target_uris(
            "\"http://ts-x-service:8000/api/\" + \"items\"",
            &AnalysisResult::default(),
        );
        assert_eq!(uris, vec!["http://ts-x-service:8000/api/items".to_string()]);
    }

    /// A variable with no binding (genuinely env-dependent) stays unresolved,
    /// so the call remains a residual -- the fix must not over-resolve.
    #[test]
    fn unbound_variable_stays_unresolved() {
        let uris = generate_target_uris("baseUrl", &AnalysisResult::default());
        assert_eq!(uris, vec!["baseUrl".to_string()]);
    }
}
