use std::sync::OnceLock;

use models::Import;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::{extractor::Extractor, queries::IMPORTS_QUERY};

pub struct ImportsExtractor;

impl Extractor<Import> for ImportsExtractor {
    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_java::LANGUAGE.into(), IMPORTS_QUERY)
                .expect("Failed to compile Java Imports Query")
        })
    }

    fn extract(&self, code: &str, tree: &tree_sitter::Tree, _file_name: &str) -> Vec<Import> {
        let query = self.query();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut imports = Vec::new();
        while let Some(m) = matches.next() {
            let mut package = String::new();
            let mut asterisk = false;

            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "import.scoped" => package = value,
                    "import.asterisk" => asterisk = true,
                    _ => {}
                }
            }
            // If there is an asterisk in the import, it is not taken
            // as a part of "import.scoped", therefore we need to know about the
            // asterisk from the "import.asterisk"
            let imported_name = if asterisk {
                "*"
            } else {
                get_package_last_identifier(&package)
            };
            imports.push(Import::from_parts(
                &package,
                imported_name,
                "",
                "",
                imported_name,
            ));
        }

        imports
    }
}

fn get_package_last_identifier(package: &str) -> &str {
    package
        .rsplit('.')
        .next()
        .expect("Import notation in Java has atleast one '.'")
}
