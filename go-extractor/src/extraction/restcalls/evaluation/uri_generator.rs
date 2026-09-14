use models::ir::ast::Expr;
use std::collections::HashMap;

use statix::symbolic::AnalysisResult;

pub fn generate_target_uris(
    template: &str,
    analysis: &AnalysisResult,
    _enums: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let resolved = substitute_template_tokens(template, analysis);
    let mut uris = vec![resolved.clone()];
    if let Some((base, _query)) = resolved.split_once('?') {
        uris.push(base.to_string());
    }
    uris.sort();
    uris.dedup();
    uris
}

fn substitute_template_tokens(template: &str, analysis: &AnalysisResult) -> String {
    let mut result = String::new();
    let mut token = String::new();

    for ch in template.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.') {
            token.push(ch);
            continue;
        }

        flush_token(&mut result, &mut token, analysis);
        result.push(ch);
    }

    flush_token(&mut result, &mut token, analysis);
    result
}

fn flush_token(result: &mut String, token: &mut String, analysis: &AnalysisResult) {
    if token.is_empty() {
        return;
    }
    result.push_str(&resolve_token(token, analysis).unwrap_or_else(|| token.clone()));
    token.clear();
}

fn resolve_token(token: &str, analysis: &AnalysisResult) -> Option<String> {
    literal_value(token, analysis)
        .or_else(|| selector_env_key(token).and_then(|env_key| literal_value(&env_key, analysis)))
}

fn literal_value(key: &str, analysis: &AnalysisResult) -> Option<String> {
    match analysis.final_env.get(key) {
        Some((_, Expr::Literal(value))) => Some(value.clone()),
        Some((_, Expr::Joined { vals })) => vals.iter().find_map(|value| match value {
            Expr::Literal(value) => Some(value.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn selector_env_key(token: &str) -> Option<String> {
    let field = token.rsplit('.').next()?;
    if !token.contains('.') || !field.chars().any(|ch| ch.is_ascii_uppercase()) {
        return None;
    }

    let mut env_key = String::new();
    for (index, ch) in field.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            env_key.push('_');
        }
        env_key.push(ch.to_ascii_uppercase());
    }
    Some(env_key)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use models::ir::ast::Expr;
    use statix::symbolic::AnalysisResult;

    use super::generate_target_uris;

    #[test]
    fn substitutes_go_config_selectors_from_scraped_env() {
        let mut env = HashMap::new();
        env.insert(
            "CUSTOMER_SERVICE_ENDPOINT".to_string(),
            (
                Some("String".to_string()),
                Expr::Literal("http://localhost:8082/api".to_string()),
            ),
        );
        let analysis = AnalysisResult {
            return_value: Expr::Empty,
            final_env: env,
        };

        let uris = generate_target_uris(
            "config.AppConfig.CustomerServiceEndpoint/customers/customerID/basketItems",
            &analysis,
            &HashMap::new(),
        );

        assert!(
            uris.contains(
                &"http://localhost:8082/api/customers/customerID/basketItems".to_string()
            )
        );
    }
}
