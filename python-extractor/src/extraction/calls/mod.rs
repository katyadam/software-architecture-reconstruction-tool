use models::CallStatement;
use tree_sitter::Node;

pub mod evaluator;
pub mod extractor;
mod type_inference;

// TODO: Rewrite!
#[derive(Debug)]
pub struct PythonCallStatement<'a> {
    pub call_statement: CallStatement,
    pub node: Node<'a>,
}

impl<'a> PythonCallStatement<'a> {
    pub fn new(call_statement: CallStatement, node: Node<'a>) -> Self {
        Self {
            call_statement,
            node,
        }
    }

    pub fn to_language_agnostic(call_statement: PythonCallStatement) -> CallStatement {
        call_statement.call_statement
    }
}
