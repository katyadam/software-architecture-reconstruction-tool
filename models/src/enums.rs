use crate::Entity;

#[derive(Debug, PartialEq, Eq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<String>,
}

impl Enum {
    pub fn new(name: String, values: Vec<String>) -> Self {
        Self { name, values }
    }

    pub fn from_entity(entity: &Entity) -> Self {
        let values = entity
            .fields
            .iter()
            .filter_map(|field| {
                field
                    .initial_value
                    .clone()
                    .map(|value| value.trim_matches('"').to_string())
            })
            .collect();
        Self {
            name: entity.name.clone(),
            values,
        }
    }
}
