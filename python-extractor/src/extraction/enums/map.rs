use std::collections::HashMap;

use models::enums::EnumDefinition;

pub fn get_enums_map(enums: &[EnumDefinition]) -> HashMap<String, Vec<String>> {
    enums
        .iter()
        .map(|enum_record| (enum_record.name.clone(), enum_record.variants.clone()))
        .collect()
}
