use std::collections::HashMap;

use models::enums::Enum;

pub fn get_enums_map(enums: &[Enum]) -> HashMap<String, Vec<String>> {
    enums
        .iter()
        .map(|enum_record| (enum_record.name.clone(), enum_record.values.clone()))
        .collect()
}
