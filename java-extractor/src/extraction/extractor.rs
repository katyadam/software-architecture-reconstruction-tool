use tree_sitter::Tree;

pub trait Extractor<T> {
    fn extract(&self, code: &str, tree: &Tree, file_name: &str) -> Vec<T>;
}
