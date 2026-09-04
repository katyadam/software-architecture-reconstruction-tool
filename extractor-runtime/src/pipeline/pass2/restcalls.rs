use java_extractor::extraction::restcalls::identification::{
    spring::SpringIdentificationStrategy, strategy::IdentificationStrategy,
};
use models::ir::{language::Language, project::TypedFileRecord};

/// Re-run REST call identification on type-resolved call statements.
///
/// Java's Spring identification requires `call.invoked_on == "RestTemplate"`, which is only
/// populated by `evaluate_invocations` (Pass 2). Pass 1 runs identification before types are
/// resolved, so `raw_restcalls` for Spring clients is empty after extraction.
pub fn re_identify_restcalls(files: &mut [TypedFileRecord]) {
    for file in files.iter_mut() {
        match file.language {
            Language::Java => identify_java(file),
            Language::Python => python_extractor::extraction::parse::identify(file),
            Language::Go => {}
        }
    }
    go_extractor::extraction::identify_project_restcalls(files);
}

fn identify_java(file: &mut TypedFileRecord) {
    let strategy = SpringIdentificationStrategy::new();
    file.raw_restcalls = file
        .call_statements
        .iter()
        .filter_map(|call| strategy.identify_restcall(call, &file.file_path))
        .collect();
}
