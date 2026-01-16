use std::collections::HashMap;

use models::RestCall;
use statix::{
    ast::{Expr, MethodAst},
    error::EvalError,
    method_match::convert_full_header_to_mangled_name,
    symbolic::SymbolicEvaluator,
};

use crate::extraction::restcalls::evaluation::strategy::EvaluationStrategy;

pub struct SpringEvaluationStrategy {
    method_asts: HashMap<String, MethodAst>,
}

impl SpringEvaluationStrategy {
    pub fn new(method_asts: HashMap<String, MethodAst>) -> Self {
        Self { method_asts }
    }
}

impl EvaluationStrategy for SpringEvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &mut RestCall) -> Result<(), EvalError> {
        let mangled_header = convert_full_header_to_mangled_name(&restcall.function_name);
        let analysis_result = SymbolicEvaluator::eval_method(&mangled_header, &self.method_asts)?;

        // 1. Split the parts and resolve each one
        let resolved_parts: Vec<String> = restcall
            .target_uri
            .split('+')
            .map(|part| {
                let part = part.trim(); // Handle potential whitespace around '+'

                // If it's a literal quote "value", strip the quotes
                if part.starts_with('"') && part.ends_with('"') {
                    part[1..part.len() - 1].to_string()
                }
                // Otherwise, treat it as a variable and look it up in the environment
                else if let Some((_, expr)) = analysis_result.final_env.get(part) {
                    if let Expr::Literal(s) = expr {
                        s.clone()
                    } else {
                        part.to_string() // Fallback if variable isn't a literal
                    }
                } else {
                    part.to_string() // Fallback if variable not found
                }
            })
            .collect();

        // 2. Rejoin the resolved parts back into the target_uri
        restcall.target_uri = resolved_parts.join("");

        Ok(())
    }
}
