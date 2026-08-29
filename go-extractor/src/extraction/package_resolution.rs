use std::collections::{HashMap, HashSet, hash_map::Entry};

use models::{
    Callable,
    ir::{language::Language, project::TypedFileRecord},
};

use super::shared::{SYNTHETIC_HANDLER_PREFIX, package_path};

pub(super) fn resolve_endpoint_handlers(files: &mut [TypedFileRecord]) {
    let package_callables = collect_package_callables(files);

    for file in files
        .iter_mut()
        .filter(|file| file.language == Language::Go)
    {
        let package = package_path(&file.file_path);
        let Some(callables) = package_callables.get(&package) else {
            continue;
        };

        let synthetic_references = file
            .callables
            .iter()
            .filter(|callable| is_synthetic_handler(&callable.metadata))
            .map(|callable| {
                (
                    callable.metadata.hash.clone(),
                    callable.metadata.name.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut resolved_synthetic_hashes = HashSet::new();

        for endpoint in &mut file.endpoints {
            let Some(reference) = synthetic_references.get(&endpoint.function_hash) else {
                continue;
            };
            let Some(callable) = resolve_reference(reference, callables) else {
                continue;
            };

            resolved_synthetic_hashes.insert(endpoint.function_hash.clone());
            endpoint.function_name = callable.signature.clone();
            endpoint.function_hash = callable.hash.clone();
        }

        file.callables
            .retain(|callable| !resolved_synthetic_hashes.contains(&callable.metadata.hash));
    }
}

fn collect_package_callables(
    files: &[TypedFileRecord],
) -> HashMap<String, HashMap<String, Option<Callable>>> {
    let mut packages = HashMap::new();
    for file in files.iter().filter(|file| file.language == Language::Go) {
        let callables = packages
            .entry(package_path(&file.file_path))
            .or_insert_with(HashMap::new);
        for parsed in &file.callables {
            let callable = &parsed.metadata;
            if is_synthetic_handler(callable) {
                continue;
            }
            insert_unique(callables, callable.name.clone(), callable.clone());
        }
    }
    packages
}

fn insert_unique(
    callables: &mut HashMap<String, Option<Callable>>,
    name: String,
    callable: Callable,
) {
    match callables.entry(name) {
        Entry::Vacant(entry) => {
            entry.insert(Some(callable));
        }
        Entry::Occupied(mut entry) => {
            if entry
                .get()
                .as_ref()
                .is_some_and(|existing| existing.hash != callable.hash)
            {
                entry.insert(None);
            }
        }
    }
}

fn resolve_reference(
    reference: &str,
    callables: &HashMap<String, Option<Callable>>,
) -> Option<Callable> {
    let reference = unwrap_handler(reference);
    let simple_name = reference.rsplit('.').next().unwrap_or(reference);
    callables.get(simple_name).and_then(Clone::clone)
}

fn unwrap_handler(reference: &str) -> &str {
    let trimmed = reference.trim().trim_end_matches("()");
    trimmed
        .strip_prefix("http.HandlerFunc(")
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(trimmed)
        .trim()
}

fn is_synthetic_handler(callable: &Callable) -> bool {
    callable.signature.starts_with(SYNTHETIC_HANDLER_PREFIX)
}
