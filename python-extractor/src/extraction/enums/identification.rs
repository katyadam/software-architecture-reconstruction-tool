use models::{Entity, enums::EnumDefinition};

#[derive(Default)]
pub struct EnumIdentificator {}

impl EnumIdentificator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identify_from_entities(entities: &[Entity]) -> Vec<EnumDefinition> {
        entities
            .iter()
            .filter(|entity| entity.superclasses.contains(&"Enum".to_string()))
            .map(EnumDefinition::from_entity)
            .collect()
    }
}
