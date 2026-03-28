use java_extractor::{
    extraction::{
        entities::{evaluator::evaluate_entity_fields, extractor::EntitiesExtractor},
        extractor::Extractor,
        imports::extractor::ImportsExtractor,
    },
    s, strs,
};
use models::{Entity, Field};

use crate::java::utils::{get_tree, load_file, parse_file};

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
                is_collection: false,
            },
            Field {
                name: s!("protectedField"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("privateField"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("packageField"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("staticField"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("FINAL_FIELD"),
                datatype: Some(s!("String")),
                initial_value: Some(s!("\"constant\"")),
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("finalInitializedInConstructor"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("tempData"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("running"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("numbers"),
                datatype: Some(s!("int[]")),
                initial_value: None,
                datatype_signature: None,
                is_collection: true,
            },
            Field {
                name: s!("names"),
                datatype: Some(s!("List<String>")),
                initial_value: None,
                datatype_signature: None,
                is_collection: true,
            },
            Field {
                name: s!("scores"),
                datatype: Some(s!("Map<String, Integer>")),
                initial_value: None,
                datatype_signature: None,
                is_collection: true,
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
                is_collection: false,
            },
            Field {
                name: s!("counter"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("runningFlag"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("tempStatic"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("name"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("age"),
                datatype: Some(s!("int")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("active"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
        ],
        signature: s!("com.java.test.AllFieldRecord"),
        file_path: s!("./examples/AllFieldRecord.java"),
    }];

    assert_eq!(entities, expected);
}

#[test]
fn test_evaluation() {
    let filename = s!("./examples/entities/ClassB.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut entities = EntitiesExtractor.extract(&code, &tree, &filename);
    let imports = ImportsExtractor.extract(&code, &tree, &filename);
    evaluate_entity_fields(&imports, &mut entities);
    let expected = vec![Entity {
        name: s!("ClassB"),
        superclasses: vec![],
        fields: vec![
            Field {
                name: s!("classC1"),
                datatype: Some(s!("ClassC")),
                initial_value: None,
                datatype_signature: Some(s!("com.example.other.ClassC")),
                is_collection: false,
            },
            Field {
                name: s!("classC2"),
                datatype: Some(s!("com.example.another.ClassC")),
                initial_value: None,
                datatype_signature: Some(s!("com.example.another.ClassC")),
                is_collection: false,
            },
            Field {
                name: s!("classWildcard"),
                datatype: Some(s!("WildCardClass")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
        ],
        signature: s!("com.example.main.ClassB"),
        file_path: s!("./examples/entities/ClassB.java"),
    }];

    assert_eq!(entities, expected);
}

#[test]
fn test_entity_with_interface_implementation() {
    let filename = s!("./examples/UserAccount.java");
    let (code, tree) = parse_file(&filename);
    let entities = EntitiesExtractor.extract(&code, &tree, &filename);

    let expected = vec![Entity {
        name: s!("UserAccount"),
        // NOTE: The entities query only captures `extends` (superclass: field in tree-sitter),
        // not `implements`. So `implements Identifiable` is NOT captured here.
        superclasses: vec![],
        fields: vec![
            Field {
                name: s!("id"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("username"),
                datatype: Some(s!("String")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
            Field {
                name: s!("active"),
                datatype: Some(s!("boolean")),
                initial_value: None,
                datatype_signature: None,
                is_collection: false,
            },
        ],
        // No package declaration in file: the extractor prepends an empty package with a dot separator
        signature: s!(".UserAccount"),
        file_path: s!("./examples/UserAccount.java"),
    }];

    assert_eq!(entities, expected);
}
