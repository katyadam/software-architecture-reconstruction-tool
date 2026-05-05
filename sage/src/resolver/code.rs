use models::ir::language::Language;

pub struct CodeSnippet {
    pub code: String,
    pub language: Language,
}

pub struct Symbol {
    pub name: String,
    pub value: Option<String>,
    pub datatype: Option<String>,
    pub kind: SymbolKind,
}

pub enum SymbolKind {
    Named,
    Imported { target_file: String },
    Attr { class: String },
}
