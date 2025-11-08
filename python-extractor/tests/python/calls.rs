use models::{Argument, CallStatement};
use python_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::{evaluator::evaluate_invocations, extractor::CallsExtractor},
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
        invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("C"),
            arguments: vec![],
            enclosing_function_name: Some(s!("D")),
            enclosing_class_name: None,
            is_self_invoke: false,
            invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("divider.divide"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Math")),
            is_self_invoke: false,
            invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
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
            invoked_on: None,
        },
    ];

    assert_eq!(calls, expected);
}

#[test]
fn should_assign_correct_invoke_on_using_assignment_type_inference() {
    let filename = s!("./examples/python/callgraph/repository-pattern/service.py");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let expected = vec![
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("ValueError"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("f\"User with email {email} already exists\""),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("User"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("id"),
                    value: s!("len(self.repository.get_all()) +\n                        1"),
                },
                Argument {
                    assigned_variable: s!("name"),
                    value: s!("name"),
                },
                Argument {
                    assigned_variable: s!("email"),
                    value: s!("email"),
                },
            ],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("len"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("self.repository.get_all"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.save"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("new_user"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_by_id"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
            }],
            enclosing_function_name: Some(s!("get_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.delete"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
            }],
            enclosing_function_name: Some(s!("delete_user")),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
    ];

    assert_eq!(calls, expected);
}

#[test]
fn should_assign_correct_invoke_on_using_function_and_assignment_type_inference() {
    let filename = s!("./examples/python/callgraph/repository-pattern/controller.py");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let expected = vec![
        CallStatement {
            function_name: s!("self.service.create_user"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("name"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("email"),
                },
            ],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserController")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("str"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("e"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserController")),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.service.get_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
            }],
            enclosing_function_name: Some(s!("get_user")),
            enclosing_class_name: Some(s!("UserController")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.list_users"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users")),
            enclosing_class_name: Some(s!("UserController")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.delete_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
            }],
            enclosing_function_name: Some(s!("delete_user")),
            enclosing_class_name: Some(s!("UserController")),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
    ];

    assert_eq!(calls, expected);
}
