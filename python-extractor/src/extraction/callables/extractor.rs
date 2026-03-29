use std::{collections::HashSet, sync::OnceLock};

use models::{Callable, Namespace, Parameter};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    callables::parser::parse_parameters,
    common::hash_text,
    extractor::{ExtractParams, Extractor},
    queries::CALLABLES_QUERY,
};

fn get_callable_header(parameters: &[Parameter]) -> String {
    let params: Vec<String> = parameters.iter().map(|p| p.to_string()).collect();
    let joined = params.join(", ");
    format!("({joined})")
}

fn get_callable_name_with_params(name: &str, parameters: &[Parameter]) -> String {
    format!("{}{}", name, get_callable_header(parameters))
}

pub struct CallablesExtractor;

impl Extractor for CallablesExtractor {
    type Item<'a> = Callable;

    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_python::LANGUAGE.into(), CALLABLES_QUERY)
                .expect("Failed to compile Python Callables Query")
        })
    }

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let query = self.query();

        let mut query_cursor = QueryCursor::new();
        let mut matches =
            query_cursor.matches(query, params.tree.root_node(), params.code.as_bytes());
        let mut callables: Vec<Callable> = vec![];
        let mut seen: HashSet<String> = HashSet::new();

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut parameters: Vec<Parameter> = vec![];
            let mut return_type: Option<String> = None;
            let mut is_async = false;
            let mut hash = String::new();
            let mut class_name: Option<String> = None;
            let mut is_constructor = false;
            m.captures.iter().for_each(|capture| {
                let value =
                    params.code[capture.node.start_byte()..capture.node.end_byte()].to_string();
                match query.capture_names()[capture.index as usize] {
                    "function.name" => {
                        name = value;
                        if name.starts_with("__init__") {
                            is_constructor = true;
                        }
                    }
                    "function.params" => {
                        parameters.extend(parse_parameters(&value));
                    }
                    "function.return_type" => {
                        return_type = Some(value);
                    }
                    "function" => {
                        if value.starts_with("async") {
                            is_async = true;
                        }
                        hash = hash_text(&value);
                    }
                    "class.name" => class_name = Some(value),
                    _ => {}
                }
            });

            if seen.contains(&hash) {
                continue;
            }
            seen.insert(hash.clone());

            let namespace = if let Some(c_name) = class_name {
                Namespace::Class(c_name)
            } else {
                Namespace::Module(params.file_name.unwrap_or_default().to_string())
            };

            let name_with_params = get_callable_name_with_params(&name, &parameters);

            let new_callable = Callable {
                name: name_with_params.clone(),
                signature: format!("{}/{}", namespace.get_signature(), name_with_params),
                namespace,
                parameters,
                return_type,
                is_async,
                is_constructor,
                hash,
                file_path: params.file_name.unwrap_or_default().to_string(),
            };

            callables.push(new_callable);
        }

        callables
    }
}
