use java_extractor::extraction::calls::evaluator::evaluate_invocations as java_evaluate_invocations;
use models::ir::{language::Language, project::TypedFileRecord};
use python_extractor::extraction::calls::evaluator::evaluate_invocations_on_statements as python_evaluate_invocations_on_statements;

use crate::pipeline::pass2::assignments::{build_cross_file_globals, merged_assignments};

/// Dispatch call type inference to the language-specific evaluator for each file,
/// using a merged map of file-local assignments supplemented by cross-file globals.
pub fn resolve_call_argument_types(files: &mut [TypedFileRecord]) {
    let cross_file_globals = build_cross_file_globals(files);

    for file in files.iter_mut() {
        let assignments = merged_assignments(&file.assignments, &cross_file_globals);
        match file.language {
            Language::Java => java_evaluate_invocations(&mut file.call_statements, &assignments),
            Language::Python => {
                python_evaluate_invocations_on_statements(&mut file.call_statements, &assignments)
            }
            Language::Unknown => continue,
        }
    }
}
