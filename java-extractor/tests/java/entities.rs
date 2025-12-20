use java_extractor::{
    extraction::{entities::extractor::EntitiesExtractor, extractor::Extractor},
    s, strs,
};
use models::{Entity, Field};

use crate::java::utils::{get_tree, load_file};

#[test]
fn base_test_class() {
    let filename = s!("./examples/AllFieldClass.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let entities = EntitiesExtractor.extract(&code, &tree, &filename);

    let expected = vec![Entity {
        name: s!("AllFieldTypes"),
        superclasses: strs!["SomeClass"],
        fields: vec![
            Field {
                name: s!("publicField"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("protectedField"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("privateField"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("packageField"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("staticField"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("FINAL_FIELD"),
                datatype: Some(s!("String")),
                initial_value: Some(s!("\"constant\"")),
                datatype_signature: None,
            },
            Field {
                name: s!("finalInitializedInConstructor"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("tempData"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("running"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("numbers"),
                datatype: Some(s!("int[]")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("names"),
                datatype: Some(s!("List<String>")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("scores"),
                datatype: Some(s!("Map<String, Integer>")),
                initial_value: None,
                datatype_signature: None,
            },
        ],
        signature: s!("com.java.test.AllFieldTypes"),
        file_path: s!("./examples/AllFieldClass.java"),
    }];

    assert_eq!(entities, expected);
}

#[test]
fn base_test_record() {
    let filename = s!("./examples/AllFieldRecord.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let entities = EntitiesExtractor.extract(&code, &tree, &filename);

    let expected = vec![Entity {
        name: s!("AllFieldRecord"),
        superclasses: vec![],
        fields: vec![
            Field {
                name: s!("VERSION"),
                datatype: Some(s!("double")),
                initial_value: Some(s!("1.0")),
                datatype_signature: None,
            },
            Field {
                name: s!("counter"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("runningFlag"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("tempStatic"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("name"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("age"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
            },
            Field {
                name: s!("active"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
            },
        ],
        signature: s!("com.java.test.AllFieldRecord"),
        file_path: s!("./examples/AllFieldRecord.java"),
    }];

    assert_eq!(entities, expected);
}
