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

/// Maps import codewords to their defining file + entity/callable.
#[derive(Debug)]
pub struct ImportGraph {
    /// codeword -> (source_file_path, fully_qualified_name)
    pub resolved_imports: HashMap<String, ResolvedImport>,
}
#[derive(Debug)]
pub struct ResolvedImport {
    pub source_file: String,
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
