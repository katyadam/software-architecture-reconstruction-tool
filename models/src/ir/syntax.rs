use std::collections::HashMap;

use crate::{
    Assignment, AssignmentKey, CallStatement, Callable, Entity, Import,
    enums::EnumDefinition,
    ir::{ast::CallableAst, language::Language},
};

pub struct FileRecord {
    pub file_path: String,
    pub language: Language,

    // Raw code elements (syntax-level, per-file)
    pub imports: Vec<Import>,
    pub entities: Vec<Entity>, // Fields have raw datatype, no datatype_signature yet
    // pub endpoints: Vec<RawEndpoint>, // URI may be incomplete (no prefix chaining)
    pub callables: Vec<Callable>,
    pub call_statements: Vec<CallStatement>, // Argument.datatype = "any" (unresolved)
    pub assignments: HashMap<AssignmentKey, Assignment>,

    // AST for symbolic evaluation in Pass 3
    pub callable_asts: HashMap<String, CallableAst>,

    // Identified enums (Python: from entities, Java: from enum declarations)
    pub enums: Vec<EnumDefinition>,
    // Pre-identified REST call candidates (before URI resolution)
    // pub raw_restcalls: Vec<RawRestCall>,
}

pub struct SyntacticIR {
    pub file_records: Vec<FileRecord>,
}
