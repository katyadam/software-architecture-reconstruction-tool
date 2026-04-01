use crate::Entity;

#[derive(Debug, PartialEq, Eq)]
pub struct EnumDefinition {
    pub name: String,
    pub variants: Vec<String>,
    pub file_path: String,
}

impl EnumDefinition {
    pub fn new(name: String, variants: Vec<String>, file_path: String) -> Self {
        Self {
            name,
            variants,
            file_path,
        }
    }

    pub fn from_entity(entity: &Entity) -> Self {
        let variants = entity
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
            variants,
            file_path: entity.file_path.clone(),
        }
    }
}
