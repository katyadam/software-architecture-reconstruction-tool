use crate::extraction::{
    entities::parser::{parse_field, parse_fields, parse_superclasses},
    extractor::{ExtractParams, Extractor},
};

use std::collections::HashSet;

use models::{Entity, Field};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::extraction::queries::ENTITIES_QUERY;

pub struct EntitiesExtractor;

impl Extractor for EntitiesExtractor {
    type Item<'a> = Entity;

    fn extract<'a>(&self, params: ExtractParams<'a>) -> Vec<Self::Item<'a>> {
        let query = Query::new(&tree_sitter_python::LANGUAGE.into(), ENTITIES_QUERY).unwrap();

        let mut query_cursor = QueryCursor::new();
        let mut matches =
            query_cursor.matches(&query, params.tree.root_node(), params.code.as_bytes());
        let mut entities: Vec<Entity> = vec![];
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(m) = matches.next() {
            let mut entity_name = String::new();
            let mut parents: Vec<String> = vec![];
            let mut fields: Vec<Field> = vec![];

            for capture in m.captures {
                let capture_text =
                    &params.code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();

                match query.capture_names()[capture.index as usize] {
                    "class.name" => entity_name = value,
                    "class.superclasses" => parents = parse_superclasses(capture.node, params.code),
                    "class.fields" => {
                        if let Some(new_field) = parse_field(&value) {
                            fields.push(new_field);
                        }
                        if value.starts_with("(") {
                            fields.extend(parse_fields(&value));
                        }
                    }
                    _ => {}
                }
            }

            if seen.contains(&entity_name) {
                continue;
            }
            seen.insert(entity_name.clone());

            let new_entity = Entity {
                name: entity_name.clone(),
                superclasses: parents,
                fields,
                signature: format!("{}/{}", params.file_name.unwrap_or_default(), entity_name),
                file_path: params.file_name.unwrap_or_default().to_string(),
            };

            entities.push(new_entity);
        }

        entities
    }
}
