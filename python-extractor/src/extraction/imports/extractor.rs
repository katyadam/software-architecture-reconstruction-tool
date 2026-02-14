use std::sync::OnceLock;

use models::Import;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    extractor::{ExtractParams, Extractor},
    queries::IMPORTS_QUERY,
};

pub struct ImportsExtractor;

impl Extractor for ImportsExtractor {
    type Item<'a> = Import;

    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_python::LANGUAGE.into(), IMPORTS_QUERY)
                .expect("Failed to compile Python Imports Query")
        })
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let query = self.query();

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut imports: Vec<Import> = vec![];
        matches.for_each(|m| {
            let mut orig_module = String::new();
            let mut module_alias = String::new();
            let mut orig_name = String::new();
            let mut name_alias = String::new();
            let mut codeword = String::new();
            m.captures.iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "import.module" => {
                        orig_module = value.clone();
                        module_alias = value;
                        codeword = orig_module.clone();
                    }
                    "aliased_import.module" => {
                        let aliased_module_import: Vec<&str> =
                            value.split(" as ").map(str::trim).collect();
                        orig_module = aliased_module_import.first().unwrap().to_string();
                        module_alias = aliased_module_import.get(1).unwrap().to_string();
                        codeword = module_alias.clone();
                    }
                    "from_import.module" => {
                        orig_module = value.clone();
                        module_alias = value;
                    }
                    "from_import.names" => {
                        orig_name = value.clone();
                        name_alias = value;
                        codeword = name_alias.clone();
                    }
                    "from_aliased_import.names" => {
                        let aliased_name: Vec<&str> = value.split(" as ").map(str::trim).collect();
                        orig_name = aliased_name.first().unwrap().to_string();
                        name_alias = aliased_name.get(1).unwrap().to_string();
                        codeword = name_alias.clone();
                    }
                    _ => {}
                }
            });
            let new_import = Import {
                orig_module,
                module_alias,
                orig_name,
                name_alias,
                codeword,
            };

            imports.push(new_import);
        });
        imports
    }
}
