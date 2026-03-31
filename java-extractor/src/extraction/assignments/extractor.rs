use std::sync::OnceLock;

use models::{Assignment, AssignmentKey, Scope};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    extraction::{
        enclosing_lookup::{get_enclosing_node_by_kind, get_field_string_from_node},
        extractor::Extractor,
        queries::ASSINGMENTS_QUERY,
    },
    parsing::parameters::parse_callable_params,
};

pub struct AssignmentsExtractor;

impl Extractor<(AssignmentKey, Assignment)> for AssignmentsExtractor {
    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_java::LANGUAGE.into(), ASSINGMENTS_QUERY)
                .expect("Failed to compile Java Assignments Query")
        })
    }

    fn extract(
        &self,
        code: &str,
        tree: &tree_sitter::Tree,
        _file_name: &str,
    ) -> Vec<(AssignmentKey, Assignment)> {
        let query = self.query();
        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut assignments_map: Vec<(AssignmentKey, Assignment)> = Vec::new();
        matches.for_each(|m| {
            let mut variable_name: Option<String> = None;
            let mut variable_value: Option<String> = None;
            let mut variable_type: Option<String> = None;
            let mut scope: Option<Scope> = None;

            let mut function_return_type: Option<String> = None;
            let mut function_name: Option<String> = None;
            let mut params_string: Option<String> = None;
            m.captures.iter().for_each(|capture| {
                let value = code[capture.node.start_byte()..capture.node.end_byte()].to_string();

                match query.capture_names()[capture.index as usize] {
                    "name" => variable_name = Some(value),
                    "type" => variable_type = Some(value),
                    "value" => variable_value = Some(value),
                    "assignment" => {
                        if let Some(fn_node) = ["method_declaration", "constructor_declaration"]
                            .iter()
                            .find_map(|kind| get_enclosing_node_by_kind(capture.node, kind))
                        {
                            if let (possible_ftype, Some(name), Some(params)) = (
                                get_field_string_from_node(fn_node, "type", code),
                                get_field_string_from_node(fn_node, "name", code),
                                get_field_string_from_node(fn_node, "parameters", code),
                            ) {
                                if let Some(ftype) = possible_ftype {
                                    scope = Some(Scope::Function(
                                        ftype.clone() + " " + &name + &params,
                                    ));
                                    function_return_type = Some(ftype);
                                } else {
                                    scope = Some(Scope::Function(name.clone() + &params));
                                }
                                function_name = Some(name);
                                params_string = Some(params);
                            }
                        } else if let Some(class_node) =
                            get_enclosing_node_by_kind(capture.node, "class_declaration")
                        {
                            if let Some(name) = get_field_string_from_node(class_node, "name", code)
                            {
                                scope = Some(Scope::Class(name));
                            }
                        } else {
                            scope = Some(Scope::Global);
                        }
                    }
                    _ => {}
                }
                if let Some(variable_name) = &variable_name {
                    let new_assignment = Assignment {
                        variable_name: variable_name.clone(),
                        variable_type: variable_type.clone().unwrap_or("any".to_owned()),
                        value: variable_value.clone().unwrap_or_default(),
                    };

                    let assignment_key = AssignmentKey {
                        scope: scope.clone().unwrap_or(Scope::Global),
                        variable_name: variable_name.clone(),
                    };
                    assignments_map.push((assignment_key, new_assignment));
                }
            });
            if let (Some(name), Some(params)) = (function_name, params_string) {
                let functions_params_assignments =
                    create_assignments_from_function_params(function_return_type, &params, &name);
                assignments_map.extend(functions_params_assignments);
            }
        });
        assignments_map
    }
}
/// Creates [`Assignment`] entries for every parameter of a function so that
/// type-inference (see [`crate::extraction::calls::type_inference`]) can resolve
/// parameter types when they are used as call-site arguments.
///
/// Each entry is placed in a [`Scope::Function`] keyed by the full method signature
/// (`"return_type name(params)"` or `"name(params)"` for constructors).
fn create_assignments_from_function_params(
    function_return_type: Option<String>,
    function_params: &str,
    function_name: &str,
) -> Vec<(AssignmentKey, Assignment)> {
    let scope = Scope::Function(match function_return_type {
        Some(ftype) => format!("{} {}{}", ftype, function_name, function_params),
        None => format!("{}{}", function_name, function_params),
    });
    let mut assignments = Vec::new();
    parse_callable_params(function_params)
        .iter()
        .for_each(|param| {
            let new_assignment = Assignment {
                variable_name: param.name.clone(),
                variable_type: param.datatype.clone().unwrap_or("any".to_string()),
                value: param.initial_value.clone().unwrap_or_default(),
            };

            let assignment_key = AssignmentKey {
                scope: scope.clone(),
                variable_name: param.name.clone(),
            };
            assignments.push((assignment_key, new_assignment));
        });
    assignments
}
