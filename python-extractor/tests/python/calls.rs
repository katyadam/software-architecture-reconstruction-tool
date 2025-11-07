use models::{Argument, CallStatement};
use python_extractor::{
    extraction::{
        calls::extractor::CallsExtractor,
        extractor::{ExtractParams, Extractor},
    },
    s,
    utils::load_file,
};

use crate::python::utils::get_tree;

#[test]
fn simple_test() {
    let filename = "./examples/python/callgraph/simple.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let expected = vec![CallStatement {
        function_name: s!("A"),
        arguments: vec![],
        enclosing_function_name: Some(s!("B")),
        enclosing_class_name: None,
        is_self_invoke: false,
    }];
    assert_eq!(calls, expected);
}

#[test]
fn nested_test() {
    let filename = "./examples/python/callgraph/nested.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let expected = vec![
        CallStatement {
            function_name: s!("func"),
            arguments: vec![],
            enclosing_function_name: Some(s!("A")),
            enclosing_class_name: None,
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("A"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("func"),
            }],
            enclosing_function_name: Some(s!("B")),
            enclosing_class_name: None,
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("B"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("C"),
            }],
            enclosing_function_name: Some(s!("D")),
            enclosing_class_name: None,
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("C"),
            arguments: vec![],
            enclosing_function_name: Some(s!("D")),
            enclosing_class_name: None,
            is_self_invoke: false,
        },
    ];

    assert_eq!(calls, expected);
}

#[test]
fn classes_test() {
    let filename = "./examples/python/callgraph/classes.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let expected = vec![
        CallStatement {
            function_name: s!("self.dividable"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Divider")),
            is_self_invoke: true,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("a"),
                    value: s!("self.a"),
                },
                Argument {
                    assigned_variable: s!("b"),
                    value: s!("self.b"),
                },
            ],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Divider")),
            is_self_invoke: false,
        },
    ];
    assert_eq!(calls, expected);
}

#[test]
fn classes_imports_test() {
    let filename = s!("./examples/python/callgraph/classes-imports.py");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));

    let expected = vec![
        CallStatement {
            function_name: s!("Divider"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                },
            ],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("divider.divide"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                },
            ],
            enclosing_function_name: Some(s!("sum")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("classes.sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("5"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("4"),
                },
            ],
            enclosing_function_name: Some(s!("product")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
        },
        CallStatement {
            function_name: s!("product"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                },
            ],
            enclosing_function_name: Some(s!("product")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
        },
    ];

    assert_eq!(calls, expected);
}
