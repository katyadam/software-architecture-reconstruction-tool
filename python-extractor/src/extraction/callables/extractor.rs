use std::collections::HashSet;

use models::{Callable, Namespace, Parameter};
use sha2::{Digest, Sha256};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::{
    callables::parser::parse_parameters,
    extractor::{ExtractParams, Extractor},
    queries::CALLABLES_QUERY,
};

pub fn get_callable_header(parameters: &Vec<Parameter>) -> String {
    let params: Vec<String> = parameters.iter().map(|p| p.to_string()).collect();
    let joined = params.join(", ");
    return format!("({})", joined);
}

pub fn get_signature(namespace: &Namespace, name: &String, parameters: &Vec<Parameter>) -> String {
    return format!(
        "{}/{}{}",
        namespace.get_signature(),
        name,
        get_callable_header(parameters)
    );
}

pub struct CallablesExtractor;

impl Extractor<Callable> for CallablesExtractor {
    fn extract(&self, params: ExtractParams) -> Vec<Callable> {
        let query = Query::new(&tree_sitter_python::LANGUAGE.into(), CALLABLES_QUERY).unwrap();

        let mut query_cursor = QueryCursor::new();
        let mut matches =
            query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut callables: Vec<Callable> = vec![];
        let mut seen: HashSet<String> = HashSet::new();

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut parameters: Vec<Parameter> = vec![];
            let mut return_type: Option<String> = None;
            let mut is_async = false;
            let mut hash = String::new();
            let mut class_name: Option<String> = None;
            m.captures.into_iter().for_each(|capture| {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "function.name" => name = value,
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
                    }
                    "function.body" => {
                        let mut hasher = Sha256::new();
                        hasher.update(value.as_bytes());
                        hash = format!("{:x}", hasher.finalize());
                    }
                    "class.name" => class_name = Some(value),
                    _ => {}
                }
            });

            if seen.contains(&name) {
                continue;
            }
            seen.insert(name.clone());

            let namespace = if let Some(c_name) = class_name {
                Namespace::Class(c_name)
            } else {
                Namespace::Module(params.file_name.unwrap_or_default().to_string())
            };

            let new_callable = Callable {
                signature: get_signature(&namespace, &name, &parameters),
                namespace: namespace,
                parameters: parameters,
                return_type: return_type,
                is_async: is_async,
                hash: hash,
            };

            callables.push(new_callable);
        }

        callables
    }
}
