use std::collections::HashMap;

use models::{
    ParsedCallable,
    ir::{
        ast::{CallableAst, Expr},
        language::Language,
        project::TypedFileRecord,
    },
};
use statix::symbolic_evaluation_with_env;

use crate::pipeline::{
    pass2::callables::mangle_callable_name,
    pass3::{env::Env, language_backend::LanguageSpecificEvaluator},
};

/// Build a per-file callable map where local definitions override globals.
pub(crate) fn build_file_local_callables(
    file: &TypedFileRecord,
    global_callables: &HashMap<String, ParsedCallable>,
) -> HashMap<String, ParsedCallable> {
    let mut map = global_callables.clone();
    for pc in &file.callables {
        let mangled = mangle_callable_name(&pc.metadata.name, file.language);
        map.insert(mangled, pc.clone());
    }
    map
}

/// Merge enum variant maps across all files; first definition wins on name collisions.
pub(crate) fn build_merged_enums(files: &[TypedFileRecord]) -> HashMap<String, Vec<String>> {
    let mut enums = HashMap::new();
    for file in files {
        for e in &file.enums {
            enums
                .entry(e.name.clone())
                .or_insert_with(|| e.variants.clone());
        }
    }
    enums
}

/// For each outer callable that has nested refs, symbolically evaluate it once
/// (with the constants env) to populate a map from inner callable key -> captured Env.
pub(super) fn build_captured_scopes<'a>(
    callables: impl Iterator<Item = (&'a str, &'a CallableAst)>,
    callables_map: &HashMap<String, ParsedCallable>,
    evaluator: &dyn LanguageSpecificEvaluator,
    constants_env: &Env,
    language: Language,
) -> HashMap<String, Env> {
    let mut captured_scopes: HashMap<String, Env> = HashMap::new();

    for (name, ast) in callables {
        if ast.nested.is_empty() {
            continue;
        }
        let mangled = mangle_callable_name(name, language);
        let Ok(result) = symbolic_evaluation_with_env(
            callables_map,
            &mangled,
            evaluator.matcher(),
            constants_env,
        ) else {
            continue;
        };

        for nested_ref in &ast.nested {
            let captured: Env = nested_ref
                .captured
                .iter()
                .filter_map(|var| result.final_env.get(var).map(|v| (var.clone(), v.clone())))
                .filter(|(_, (_, expr))| *expr != Expr::Empty)
                .collect();

            if !captured.is_empty() {
                // Key by hash (unique per function body) so sibling inner functions
                // with the same mangled name get distinct entries. Matches the
                // lookup key `restcall.function_hash` used above.
                captured_scopes
                    .entry(nested_ref.hash.clone())
                    .or_insert(captured);
            }
        }
    }

    captured_scopes
}
