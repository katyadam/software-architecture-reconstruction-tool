use models::{Entity, enums::Enum};

pub struct EnumIdentificator {}

impl EnumIdentificator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identificate_from_entities(entities: &[Entity]) -> Vec<Enum> {
        entities
            .iter()
            .filter(|entity| entity.superclasses.contains(&"Enum".to_string()))
            .map(|entity| Enum::from_entity(&entity))
            .collect()
    }
}
