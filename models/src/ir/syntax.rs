use std::collections::HashMap;

use crate::{
    Assignment, AssignmentKey, CallStatement, Endpoint, Entity, Import, ParsedCallable,
    enums::EnumDefinition, ir::language::Language,
};

/// Pass 1 output: one per source file.
///
/// Carries syntax only. REST calls and message edges are identified in Pass 2,
/// once types are resolved, and live on `TypedFileRecord`.
pub struct FileRecord {
    pub file_path: String,
    pub language: Language,

    // Raw code elements (syntax-level, per-file)
    pub imports: Vec<Import>,
    pub entities: Vec<Entity>, // Fields have raw datatype, no datatype_signature yet
    pub endpoints: Vec<Endpoint>, // URI may be incomplete (no prefix chaining)
    pub callables: Vec<ParsedCallable>,
    pub call_statements: Vec<CallStatement>, // Argument.datatype = "any" (unresolved)
    pub assignments: HashMap<AssignmentKey, Assignment>,

    // Identified enums (Python: from entities, Java: from enum declarations)
    pub enums: Vec<EnumDefinition>,
}

pub struct SyntacticIR {
    pub file_records: Vec<FileRecord>,
}
