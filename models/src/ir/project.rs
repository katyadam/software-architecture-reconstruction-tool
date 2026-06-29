use std::collections::HashMap;

use crate::{
    Assignment, AssignmentKey, CallStatement, Endpoint, Entity, Import, ParsedCallable, RestCall,
    enums::EnumDefinition,
    ir::{language::Language, syntax::FileRecord},
};

pub struct ProjectIR {
    pub files: Vec<TypedFileRecord>,
    pub import_graph: ImportGraph,
    pub class_hierarchy: ClassHierarchy,
    pub constants: HashMap<String, ConstantValue>,
    /// Callables keyed by *mangled name* -- the lookup when you know the symbol.
    pub callable_map: HashMap<String, ParsedCallable>,
    /// Callables keyed by `(file_path, content-hash)` -- the lookup when you know
    /// a call site. Companion to [`Self::callable_map`]: `function_hash` is a
    /// *content* hash (identical bodies collide across files), so it is paired
    /// with `file_path` to scope the lookup to the owning file.
    pub callables_by_file_hash: HashMap<(String, String), ParsedCallable>,
}

impl ProjectIR {
    /// Resolve the callable enclosing a call site by its `(file_path, hash)`.
    /// Returns `None` for an empty hash or a miss. Safe against cross-file hash
    /// collisions because the key is scoped by `file_path`.
    pub fn enclosing_callable(&self, file_path: &str, hash: &str) -> Option<&ParsedCallable> {
        if hash.is_empty() {
            return None;
        }
        self.callables_by_file_hash
            .get(&(file_path.to_string(), hash.to_string()))
    }
}

/// A FileRecord with resolved types.
pub struct TypedFileRecord {
    pub file_path: String,
    pub language: Language,

    pub imports: Vec<Import>,
    pub entities: Vec<Entity>,    // Field.datatype_signature NOW resolved
    pub endpoints: Vec<Endpoint>, // Still may need prefix resolution
    pub callables: Vec<ParsedCallable>,
    pub call_statements: Vec<CallStatement>, // Argument.datatype NOW resolved where possible
    pub assignments: HashMap<AssignmentKey, Assignment>,

    pub enums: Vec<EnumDefinition>,
    pub raw_restcalls: Vec<RestCall>,
}

/// Maps (importer_file_path, codeword) pairs to their resolved definition.
///
/// Keying on the importer prevents cross-microservice collisions: two files in
/// different services that both import a symbol named `Order` get independent
/// slots even if the plain codeword would otherwise clash.
#[derive(Debug)]
pub struct ImportGraph {
    pub resolved_imports: HashMap<(String, String), ResolvedImport>,
}

impl ImportGraph {
    /// Look up what `codeword` resolves to when used inside `importer_file`.
    pub fn lookup(&self, importer_file: &str, codeword: &str) -> Option<&ResolvedImport> {
        self.resolved_imports
            .get(&(importer_file.to_string(), codeword.to_string()))
    }
}
#[derive(Debug)]
pub struct ResolvedImport {
    /// Absolute or project-relative path to the file that defines the imported symbol.
    pub source_file: String,
    /// Path-style identifier of the symbol in `source_file`, e.g. `"medical-data-service/base_url"`
    /// for `from singletons import base_url` in that service. Always a slash-separated path ending
    /// in the symbol name as defined in the source, never the local import alias.
    pub fully_qualified_name: String,
    pub kind: ImportKind,
}

#[derive(Debug)]
pub enum ImportKind {
    Entity,
    Callable,
    Module,
    Constant,
}

/// Entity inheritance and interface relationships across files.
pub struct ClassHierarchy {
    /// entity_signature -> list of parent entity_signatures (resolved via imports)
    pub parents: HashMap<String, Vec<String>>,
    /// entity_signature -> list of child entity_signatures
    pub children: HashMap<String, Vec<String>>,
}

pub struct ConstantValue {
    pub name: String,
    pub value: String,
    pub source_file: String,
}

impl From<FileRecord> for TypedFileRecord {
    fn from(r: FileRecord) -> Self {
        TypedFileRecord {
            file_path: r.file_path,
            language: r.language,
            imports: r.imports,
            entities: r.entities,
            endpoints: r.endpoints,
            callables: r.callables,
            call_statements: r.call_statements,
            assignments: r.assignments,
            enums: r.enums,
            raw_restcalls: r.raw_restcalls,
        }
    }
}
