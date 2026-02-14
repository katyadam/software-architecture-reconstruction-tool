use std::collections::HashMap;

use models::{Assignment, AssignmentKey};
use tree_sitter::Tree;

use crate::extraction::{assignments::extractor::AssignmentsExtractor, extractor::Extractor};

pub fn get_assignments_map(tree: &Tree, code: &str) -> HashMap<AssignmentKey, Assignment> {
    let extractor = AssignmentsExtractor;
    let raw_assignments = extractor.extract(code, tree, "");

    raw_assignments
        .into_iter()
        .fold(HashMap::new(), |mut acc, (key, incoming)| {
            acc.entry(key)
                .and_modify(|existing| {
                    // If there are multiple assignments to the same variable,
                    // we want to overaproximate the resulting assignment.
                    // For example, int a; and a = 2; will result to single assignment of
                    // variable "a" with datatype "int" and value "2"
                    if existing.value.is_empty() {
                        existing.value = incoming.value.clone();
                    }
                    if existing.variable_type == "any" {
                        existing.variable_type = incoming.variable_type.clone();
                    }
                })
                .or_insert(incoming);
            acc
        })
}
