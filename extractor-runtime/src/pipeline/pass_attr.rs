//! Pass-attr: resolves `var.field` attribute accesses to class-field defaults.
//!
//! For each global-scope binding `var = ClassName()`, resolves `ClassName` via the
//! `ImportGraph` to an `Entity`, then emits `"var.field_name" -> default_value` pairs
//! keyed by the importer file path. Pass 3 layers these below project constants and
//! CLI overrides so that `settings.cds_url` resolves without manual `constants.json`.

use std::collections::HashMap;

use models::{
    Entity, Scope,
    ir::project::{ImportKind, ProjectIR, TypedFileRecord},
};
use once_cell::sync::Lazy;
use regex::Regex;

/// Dotted attribute key (`"settings.cds_url"`) -> stripped literal default.
type AttrMap = HashMap<String, String>;

/// Per-file attribute map: importer file path -> [`AttrMap`].
pub type PerFileAttrMap = HashMap<String, AttrMap>;

/// Temporary index built once per `resolve_all` call for O(1) file lookups.
type FileIndex<'a> = HashMap<&'a str, &'a TypedFileRecord>;

/// Regex matching a bare constructor call at the start of an assignment value.
/// Captures the class name from expressions like `Settings()` or `MyClass( )`.
static CONSTRUCTOR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*\(").expect("valid regex literal"));

/// Intermediate result of Stage 1 — a global-scope instance binding.
struct Binding<'a> {
    /// Absolute path of the file containing the binding assignment.
    importer_file: &'a str,
    /// Variable name on the left-hand side (e.g. `"settings"`).
    var_name: &'a str,
    /// Class name used on the right-hand side (e.g. `"Settings"`).
    class_ref: String,
}

/// Resolve attribute defaults from class field declarations across the whole project.
///
/// For each global-scope assignment of the form `var = ClassName()` (or the
/// annotated form `var: ClassName = ClassName()`), this function:
///
/// 1. Collects candidate bindings (Stage 1).
/// 2. Resolves each binding's class reference to an `Entity` via `ImportGraph`
///    (Stage 2), falling back to a local entity lookup when no import is found.
/// 3. Emits `"var.field_name" -> literal_default` pairs for every field that
///    carries a non-empty literal default (Stage 3).
///
/// Returns a two-level map: outer key is the importer file path; inner key is
/// the dotted attribute path (e.g. `"settings.cds_url"`); value is the stripped
/// literal default.
pub fn resolve_all(project_ir: &ProjectIR) -> PerFileAttrMap {
    // Stage 1: collect candidate bindings from global-scope assignments.
    let bindings = collect_bindings(project_ir);

    // Index files by path once so Stage 2 lookups are O(1).
    let file_index: FileIndex<'_> = project_ir
        .files
        .iter()
        .map(|f| (f.file_path.as_str(), f))
        .collect();

    // Stage 2+3: resolve each binding to an entity and emit dotted constants.
    let mut result = PerFileAttrMap::new();

    for binding in &bindings {
        let Some(entity) = resolve_entity(binding, project_ir, &file_index) else {
            continue;
        };

        let bucket = result.entry(binding.importer_file.to_string()).or_default();

        emit_field_defaults(binding.var_name, entity, bucket);
    }

    result
}

/// Stage 1 — iterate all files and collect global-scope constructor bindings.
fn collect_bindings<'a>(project_ir: &'a ProjectIR) -> Vec<Binding<'a>> {
    let mut bindings = Vec::new();

    for file in &project_ir.files {
        for (key, assignment) in &file.assignments {
            if !matches!(key.scope, Scope::Global) {
                continue;
            }

            let class_ref = derive_class_ref(assignment);
            if let Some(class_ref) = class_ref {
                bindings.push(Binding {
                    importer_file: &file.file_path,
                    var_name: &key.variable_name,
                    class_ref,
                });
            }
        }
    }

    bindings
}

/// Derive the class reference from an assignment.
///
/// Priority:
/// 1. Non-empty `variable_type` (covers annotated bindings like `var: MyClass = MyClass()`).
/// 2. Regex match on `value` for bare constructor calls like `MyClass()`.
/// 3. `None` if neither applies.
fn derive_class_ref(assignment: &models::Assignment) -> Option<String> {
    if !assignment.variable_type.is_empty() {
        return Some(assignment.variable_type.clone());
    }

    let captures = CONSTRUCTOR_RE.captures(assignment.value.trim())?;
    captures.get(1).map(|m| m.as_str().to_string())
}

/// Stage 2 — resolve a `Binding` to its class `Entity`.
///
/// Lookup order:
/// 1. `ImportGraph::lookup` in the importer file — if resolved to `ImportKind::Entity`,
///    find the `TypedFileRecord` for the source file and locate the entity there.
/// 2. Local fallback — scan the importer file's own entities for a name match.
fn resolve_entity<'a>(
    binding: &Binding<'_>,
    project_ir: &'a ProjectIR,
    file_index: &FileIndex<'a>,
) -> Option<&'a Entity> {
    // Primary: try import graph resolution.
    if let Some(resolved) = project_ir
        .import_graph
        .lookup(binding.importer_file, &binding.class_ref)
        .filter(|r| matches!(r.kind, ImportKind::Entity))
        && let Some(source_record) = file_index.get(resolved.source_file.as_str())
    {
        // Prefer signature match; fall back to name match.
        let entity = source_record
            .entities
            .iter()
            .find(|e| e.signature == resolved.fully_qualified_name)
            .or_else(|| {
                source_record
                    .entities
                    .iter()
                    .find(|e| e.name == binding.class_ref)
            });

        if entity.is_some() {
            return entity;
        }
    }

    // Fallback: look for a locally-defined entity in the importer file.
    let importer_record = file_index.get(binding.importer_file)?;
    importer_record
        .entities
        .iter()
        .find(|e| e.name == binding.class_ref)
}

/// Stage 3 — emit `"var_name.field_name" -> stripped_value` into `bucket`.
///
/// Skips fields with no `initial_value` and fields whose value is empty after
/// stripping surrounding single or double quotes.
fn emit_field_defaults(var_name: &str, entity: &Entity, bucket: &mut AttrMap) {
    for field in &entity.fields {
        let Some(raw_value) = &field.initial_value else {
            continue;
        };

        let stripped = raw_value
            .trim_matches(|c: char| c == '"' || c == '\'')
            .to_string();

        if stripped.is_empty() {
            continue;
        }

        bucket
            .entry(format!("{}.{}", var_name, field.name))
            .or_insert(stripped);
    }
}
