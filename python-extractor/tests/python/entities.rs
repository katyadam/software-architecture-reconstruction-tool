use python_extractor::{
    extraction::{
        entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
        extractor::{ExtractParams, Extractor},
        imports::extractor::ImportsExtractor,
    },
    s, strs,
    utils::load_file,
};

use models::{Entity, Field};

use crate::python::utils::get_tree;

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
                },
                Field {
                    name: s!("domain"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
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
                },
                Field {
                    name: s!("name"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                },
                Field {
                    name: s!("description"),
                    datatype: Some(s!("Optional[str]")),
                    initial_value: Some(s!("description")),
                    datatype_signature: None,
                },
                Field {
                    name: s!("price"),
                    datatype: Some(s!("float")),
                    initial_value: None,
                    datatype_signature: None,
                },
                Field {
                    name: s!("in_stock"),
                    datatype: Some(s!("bool")),
                    initial_value: None,
                    datatype_signature: None,
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
                },
                Field {
                    name: s!("description"),
                    datatype: Some(s!("Optional[str]")),
                    initial_value: None,
                    datatype_signature: None,
                },
                Field {
                    name: s!("price"),
                    datatype: Some(s!("float")),
                    initial_value: None,
                    datatype_signature: None,
                },
                Field {
                    name: s!("in_stock"),
                    datatype: Some(s!("bool")),
                    initial_value: Some(s!("in_stock")),
                    datatype_signature: None,
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
                },
                Field {
                    name: s!("username"),
                    datatype: Some(s!("str")),
                    initial_value: None,
                    datatype_signature: None,
                },
                Field {
                    name: s!("email"),
                    datatype: Some(s!("Email")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities.py/Email")),
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
                },
                Field {
                    name: s!("email"),
                    datatype: Some(s!("Email")),
                    initial_value: None,
                    datatype_signature: Some(s!("./examples/python/entities.py/Email")),
                },
            ],
            signature: s!("./examples/python/entities.py/UserCreate"),
            file_path: s!(filename),
        },
    ];

    assert_eq!(entities, expected);
}
