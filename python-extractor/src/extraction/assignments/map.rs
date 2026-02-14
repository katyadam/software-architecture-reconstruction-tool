use std::collections::HashMap;

use models::{Assignment, AssignmentKey};
use tree_sitter::Tree;

use crate::extraction::{
    assignments::extractor::AssignmentsExtractor,
    extractor::{ExtractParams, Extractor},
};

pub fn get_assignments_map(tree: &Tree, code: &str) -> HashMap<AssignmentKey, Assignment> {
    let extractor = AssignmentsExtractor;

    extractor
        .extract(ExtractParams::new(tree, code))
        .into_iter()
        .collect()
}
