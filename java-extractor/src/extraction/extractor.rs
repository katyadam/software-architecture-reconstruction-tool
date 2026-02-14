use tree_sitter::{Query, Tree};

pub trait Extractor<T> {
    fn query(&self) -> &'static Query;

    fn extract(&self, code: &str, tree: &Tree, file_name: &str) -> Vec<T>;
}
