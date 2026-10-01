use std::collections::HashMap;

use models::{ParsedCallable, Scope, ir::language::Language, ir::project::TypedFileRecord};

use super::{identify_with_package_context, shared::package_path};

#[derive(Default)]
struct PackageContext {
    globals: HashMap<String, String>,
    callables: Vec<ParsedCallable>,
}

pub(super) fn identify_restcalls(files: &mut [TypedFileRecord]) {
    let contexts = collect_package_contexts(files);

    for file in files
        .iter_mut()
        .filter(|file| file.language == Language::Go)
    {
        let Some(context) = contexts.get(&package_path(&file.file_path)) else {
            continue;
        };
        identify_with_package_context(file, &context.globals, &context.callables);
    }
}

fn collect_package_contexts(files: &[TypedFileRecord]) -> HashMap<String, PackageContext> {
    let mut packages = HashMap::new();
    for file in files.iter().filter(|file| file.language == Language::Go) {
        let context = packages
            .entry(package_path(&file.file_path))
            .or_insert_with(PackageContext::default);
        context.callables.extend(file.callables.iter().cloned());
        for (key, assignment) in &file.assignments {
            if key.scope == Scope::Global {
                context
                    .globals
                    .insert(assignment.variable_name.clone(), assignment.value.clone());
            }
        }
    }
    packages
}
