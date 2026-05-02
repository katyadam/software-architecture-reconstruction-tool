//! Synthesize a `<module>` [`ParsedCallable`] from a Python file's top-level
//! statements so that the symbolic evaluator can propagate module-level
//! assignments the same way it propagates function-local ones.

use models::{Callable, Namespace, ParsedCallable};
use statix::python::parser::parse_python_module_body;
use tree_sitter::Tree;

pub const MODULE_CALLABLE_NAME: &str = "<module>()";

/// Build the `<module>` synthetic callable for a Python file.
///
/// The callable's AST contains every top-level statement the evaluator can reason
/// about (assignments, declarations, `if`, `try`), mirroring the shape produced
/// for real function bodies. Every module callable carries a `None` return type
/// so its mangled key is stable across `build_file_local_callables` call sites.
pub fn build_module_callable(tree: &Tree, code: &str, file_name: &str) -> ParsedCallable {
    let ast = parse_python_module_body(tree.root_node(), code);

    let metadata = Callable {
        name: MODULE_CALLABLE_NAME.to_string(),
        signature: format!("module:{file_name}/{MODULE_CALLABLE_NAME}"),
        namespace: Namespace::Module(file_name.to_string()),
        parameters: vec![],
        return_type: None,
        is_async: false,
        is_constructor: false,
        hash: statix::strings::hash_text(file_name),
        file_path: file_name.to_string(),
    };

    ParsedCallable { metadata, ast }
}
