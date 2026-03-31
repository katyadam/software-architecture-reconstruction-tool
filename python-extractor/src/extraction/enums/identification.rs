use models::{Entity, enums::Enum};

#[derive(Default)]
pub struct EnumIdentificator {}

impl EnumIdentificator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn identify_from_entities(entities: &[Entity]) -> Vec<Enum> {
        entities
            .iter()
            .filter(|entity| entity.superclasses.contains(&"Enum".to_string()))
            .map(Enum::from_entity)
            .collect()
    }
}
