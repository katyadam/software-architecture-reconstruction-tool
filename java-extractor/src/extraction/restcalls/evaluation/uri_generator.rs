use statix::{ast::Expr, symbolic::AnalysisResult};

type ResolvedPart = Vec<String>;

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
