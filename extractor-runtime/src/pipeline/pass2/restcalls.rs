use java_extractor::extraction::restcalls::identification::{
    spring::SpringIdentificationStrategy, strategy::IdentificationStrategy,
};
use std::{collections::HashMap, path::Path};
use models::ir::{language::Language, project::TypedFileRecord};

/// Re-run REST call identification on type-resolved call statements.
///
/// Java's Spring identification requires `call.invoked_on == "RestTemplate"`, which is only
/// populated by `evaluate_invocations` (Pass 2). Pass 1 runs identification before types are
/// resolved, so `raw_restcalls` for Spring clients is empty after extraction.
pub fn re_identify_restcalls(files: &mut [TypedFileRecord]) {
    let strategy = SpringIdentificationStrategy::new();
    let go_package_globals = collect_go_package_globals(files);
    for file in files.iter_mut() {
        match file.language {
            Language::Java => {
                let identified: Vec<_> = file
                    .call_statements
                    .iter()
                    .filter_map(|call| strategy.identify_restcall(call, &file.file_path))
                    .collect();
                file.raw_restcalls.extend(identified);
            }
            Language::Go => {
                let package_dir = parent_dir(&file.file_path);
                let globals = go_package_globals.get(package_dir.as_str()).cloned().unwrap_or_default();
                go_extractor::extraction::identify_with_package_globals(file, &globals);
            }
            Language::Python => {}
        }
    }
}

fn collect_go_package_globals(files: &[TypedFileRecord]) -> HashMap<String, HashMap<String, String>> {
    let mut packages = HashMap::new();
    for file in files.iter().filter(|file| file.language == Language::Go) {
        let globals = packages.entry(parent_dir(&file.file_path)).or_insert_with(HashMap::new);
        for (key, assignment) in &file.assignments {
            if key.scope == models::Scope::Global {
                globals.insert(assignment.variable_name.clone(), assignment.value.clone());
            }
        }
    }
    packages
}

fn parent_dir(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}
