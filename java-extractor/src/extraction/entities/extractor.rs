use tree_sitter::{Query, Tree};

use std::{collections::HashSet, sync::OnceLock};

use crate::extraction::{
    entities::parser::{parse_field_declarations, parse_formal_parameters},
    extractor::Extractor,
    queries::ENTITIES_QUERY,
};
use models::{Entity, Field};
use tree_sitter::{QueryCursor, StreamingIterator};

pub struct EntitiesExtractor;

impl Extractor<Entity> for EntitiesExtractor {
    fn query(&self) -> &'static Query {
        static QUERY: OnceLock<Query> = OnceLock::new();

        QUERY.get_or_init(|| {
            Query::new(&tree_sitter_java::LANGUAGE.into(), ENTITIES_QUERY)
                .expect("Failed to compile Java Entities Query")
        })
    }

    fn extract(&self, code: &str, tree: &Tree, file_name: &str) -> Vec<Entity> {
        let query = self.query();
        let mut query_cursor = QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), code.as_bytes());
        let mut entities: Vec<Entity> = vec![];
        let mut seen: HashSet<String> = HashSet::new();
        let mut package: Option<String> = None;

        while let Some(m) = matches.next() {
            let mut entity_name = String::new();
            let mut superclasses: Vec<String> = vec![];
            let mut fields: Vec<Field> = vec![];
            let mut record_fields: Vec<Field> = vec![];

            for capture in m.captures {
                let capture_text =
                    &code.as_bytes()[capture.node.start_byte()..capture.node.end_byte()];
                let value = String::from_utf8_lossy(capture_text).to_string();
                match query.capture_names()[capture.index as usize] {
                    "entity.name" => entity_name = value,
                    "entity.superclass" => superclasses.push(value),
                    "entity.body" => {
                        fields = parse_field_declarations(capture.node, code);
                    }
                    "entity.recordparams" => {
                        record_fields = parse_formal_parameters(capture.node, code);
                    }
                    "package" => {
                        package = Some(value);
                    }
                    _ => {}
                }
            }

            if seen.contains(&entity_name) || entity_name.is_empty() {
                continue;
            }
            seen.insert(entity_name.clone());

            fields.append(&mut record_fields);
            let new_entity = Entity {
                name: entity_name.clone(),
                superclasses,
                fields,
                signature: format!("{}.{}", package.clone().unwrap_or_default(), entity_name),
                file_path: file_name.to_string(),
            };

            entities.push(new_entity);
        }

        entities
    }
}
