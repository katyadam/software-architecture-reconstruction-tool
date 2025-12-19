use models::Import;

use crate::extraction::extractor::Extractor;

pub struct ImportsExtractor;

impl Extractor<Import> for ImportsExtractor {
    fn extract(&self, code: &str, tree: &tree_sitter::Tree, file_name: &str) -> Vec<Import> {
        // TODO: extract package from each java file and then assign the specific class in that java file with the correct package/path name
        vec![]
    }
}
