use std::collections::HashMap;

use models::ir::{ast::Expr, project::ConstantValue};

/// Convert project constants to the initial symbolic evaluation environment.
///
/// Quoted string values have their surrounding quotes stripped so that
/// `BASE_URL = "/api/v1"` becomes `Expr::Literal("/api/v1")`.
///
/// All constants are assigned dtype `"String"` regardless of their actual type.
/// Numeric and boolean constants are represented correctly as `Expr::Literal` values
/// but carry a `String` dtype, which is intentional for URI concat purposes.
pub(crate) fn constants_to_env(
    constants: &HashMap<String, ConstantValue>,
) -> HashMap<String, (Option<String>, Expr)> {
    constants
        .iter()
        .map(|(name, cv)| {
            let value = cv.value.trim_matches(|c| c == '"' || c == '\'').to_string();
            (
                name.clone(),
                (Some("String".to_string()), Expr::Literal(value)),
            )
        })
        .collect()
}
