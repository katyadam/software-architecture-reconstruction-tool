use models::{Entity, Field};

pub fn test_entity_email() -> Entity {
    Entity {
        name: "Email".to_string(),
        superclasses: vec!["BaseModel".to_string()],
        fields: vec![
            Field {
                name: "username".to_string(),
                datatype: Some("str".to_string()),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: "domain".to_string(),
                datatype: Some("str".to_string()),
                initial_value: None,
                datatype_signature: None,
            },
        ],
        signature: "./examples/python/entities.py/Email".to_string(),
        file_path: "./examples/python/entities.py".to_string(),
    }
}
