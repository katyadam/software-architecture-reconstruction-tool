use python_extractor::{
    extraction::{
        entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
        enums::identification::EnumIdentificator,
        extractor::{ExtractParams, Extractor},
        imports::extractor::ImportsExtractor,
    },
    s, strs,
};

use models::{Entity, Field, enums::Enum};

use crate::python::utils::{get_tree, load_file};

#[test]
fn base_test() {
    let filename = "./examples/python/entities.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut entities =
        EntitiesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));
    let imports = ImportsExtractor.extract(ExtractParams::new(&tree, &code));
    evaluate_entity_fields(&imports, &mut entities, &filename);
    let expected = vec![
        Entity {
            name: s!("Email"),
            superclasses: strs!["BaseModel"],
            fields: vec![
                Field {
                    name: s!("username"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("domain"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities.py/Email"),
            file_path: s!(filename),
        },
        Entity {
            name: s!("Item2"),
            superclasses: strs!["BaseModel", "Else"],
            fields: vec![],
            signature: s!("./examples/python/entities.py/Item2"),
            file_path: s!(filename),
        },
        Entity {
            name: s!("Item"),
            superclasses: strs!["BaseModel"],
            fields: vec![
                Field {
                    name: s!("id"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("name"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("description"),
                    datatype: Some(s!("Optional[str]")),
                    initial_value: Some(s!("None")),
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("price"),
                    datatype: Some(s!("float")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("in_stock"),
                    datatype: Some(s!("bool")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities.py/Item"),
            file_path: s!(filename),
        },
        Entity {
            name: s!("ItemCreate"),
            superclasses: strs!["BaseModel"],
            fields: vec![
                Field {
                    name: s!("name"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("description"),
                    datatype: Some(s!("Optional[str]")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("price"),
                    datatype: Some(s!("float")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("in_stock"),
                    datatype: Some(s!("bool")),
                    initial_value: Some(s!("True")),
                    datatype_signature: None,
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities.py/ItemCreate"),
            file_path: s!(filename),
        },
        Entity {
            name: s!("User"),
            superclasses: strs!["BaseModel"],
            fields: vec![
                Field {
                    name: s!("id"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("username"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("email"),
                    datatype: Some(s!("Email")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities.py/Email")),
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities.py/User"),
            file_path: s!(filename),
        },
        Entity {
            name: s!("UserCreate"),
            superclasses: strs!["BaseModel"],
            fields: vec![
                Field {
                    name: s!("username"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("email"),
                    datatype: Some(s!("Email")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities.py/Email")),
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities.py/UserCreate"),
            file_path: s!(filename),
        },
    ];

    assert_eq!(entities, expected);
}

#[test]
fn should_identificate_field_datatype_collections() {
    let filename = "./examples/python/entities_collections.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let mut entities =
        EntitiesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));
    let imports = ImportsExtractor.extract(ExtractParams::new(&tree, &code));
    evaluate_entity_fields(&imports, &mut entities, &filename);
    let expected = vec![
        Entity {
            name: s!("Email"),
            superclasses: vec![s!("BaseModel")],
            fields: vec![
                Field {
                    name: s!("username"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
                Field {
                    name: s!("domain"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                    is_collection: false,
                },
            ],
            signature: s!("./examples/python/entities_collections.py/Email"),
            file_path: s!("./examples/python/entities_collections.py"),
        },
        Entity {
            name: s!("MailService"),
            superclasses: vec![],
            fields: vec![
                Field {
                    name: s!("emails"),
                    datatype: Some(s!("List[Email]")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities_collections.py/Email")),
                    is_collection: true,
                },
                Field {
                    name: s!("emails2"),
                    datatype: Some(s!("list[Email]")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities_collections.py/Email")),
                    is_collection: true,
                },
            ],
            signature: s!("./examples/python/entities_collections.py/MailService"),
            file_path: s!("./examples/python/entities_collections.py"),
        },
    ];
    assert_eq!(entities, expected);
}

#[test]
fn should_identificate_enums() {
    let filename = "./examples/python/enums.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let entities =
        EntitiesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));
    let enums = EnumIdentificator::identify_from_entities(&entities);
    let expected = vec![Enum {
        name: s!("MappingType"),
        values: vec![s!("cases"), s!("slides")],
    }];
    assert_eq!(enums, expected);
}
