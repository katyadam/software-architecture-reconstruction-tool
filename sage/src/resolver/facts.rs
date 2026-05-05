use models::ir::{language::Framework, project::ConstantValue};

use crate::resolver::{
    code::{CodeSnippet, Symbol},
    messages::Message,
};

pub struct FactBundle {
    pub sites: Vec<CodeSnippet>,
    pub frameworks: Vec<Framework>,
    pub local_scope: Vec<Symbol>,
    pub imported_scope: Vec<Symbol>,
    pub class_or_module_attrs: Vec<Symbol>,
    pub constants: Vec<ConstantValue>,
    pub others: Vec<Message>,
}
