mod endpoint_frameworks;
mod endpoints;
mod evaluator;
mod files;
mod identify;
mod imports;
mod ir;
mod message_edges;
mod package_resolution;
mod project;
pub mod restcalls;
mod shared;

use std::{collections::HashMap, path::Path};

use models::{
    api::ExtractionError,
    assignments::Scope,
    ir::{language::Language, project::TypedFileRecord, syntax::FileRecord},
};
use tree_sitter::{Parser, Tree};

pub fn should_extract_file(path: &Path) -> bool {
    !files::is_generated(path)
}

pub fn extract_syntactic(text: &str, file_path: &str) -> Result<FileRecord, ExtractionError> {
    let tree = parse_go_tree(text)?;
    let root = tree.root_node();

    let mut callables = Vec::new();
    let mut callable_lookup = HashMap::new();
    let mut call_statements = Vec::new();
    let mut assignments = HashMap::new();
    let imports = imports::collect_imports(root, text);

    ir::collect_global_assignments(root, text, &mut assignments);
    ir::collect_callable_ir(
        root,
        text,
        file_path,
        &mut callables,
        &mut callable_lookup,
        &mut call_statements,
        &mut assignments,
    );

    let mut synthetic_callables = Vec::new();
    let endpoints = endpoints::collect_endpoints(
        root,
        text,
        file_path,
        &assignments,
        &imports,
        &callable_lookup,
        &mut synthetic_callables,
    );
    callables.extend(synthetic_callables);

    Ok(FileRecord {
        file_path: file_path.to_string(),
        language: Language::Go,
        imports,
        entities: vec![],
        endpoints,
        callables,
        call_statements,
        assignments,
        enums: vec![],
        raw_message_edges: vec![],
    })
}

pub fn identify(file: &mut TypedFileRecord) {
    let globals = file
        .assignments
        .iter()
        .filter(|(key, _)| key.scope == Scope::Global)
        .map(|(_, assignment)| (assignment.variable_name.clone(), assignment.value.clone()))
        .collect::<HashMap<_, _>>();
    let callables = file.callables.clone();
    identify_with_package_context(file, &globals, &callables);
}

pub fn identify_with_package_context(
    file: &mut TypedFileRecord,
    package_globals: &HashMap<String, String>,
    package_callables: &[models::ParsedCallable],
) {
    file.raw_restcalls = file
        .call_statements
        .iter()
        .filter_map(|call| {
            identify::identify_restcall(file, call, Some(package_globals), package_callables)
        })
        .collect();
    file.raw_message_edges = file
        .call_statements
        .iter()
        .filter_map(|call| message_edges::identify_message_edge(call, &file.file_path))
        .collect();
}

pub fn resolve_package_endpoint_handlers(files: &mut [TypedFileRecord]) {
    package_resolution::resolve_endpoint_handlers(files);
}

pub fn identify_project_restcalls(files: &mut [TypedFileRecord]) {
    project::identify_restcalls(files);
}

fn parse_go_tree(code: &str) -> Result<Tree, ExtractionError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|err| ExtractionError::Process(format!("failed to load Go grammar: {err}")))?;
    parser
        .parse(code, None)
        .ok_or_else(|| ExtractionError::Process("failed to parse Go source".to_string()))
}

#[cfg(test)]
mod tests;
