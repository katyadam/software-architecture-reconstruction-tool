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
        enclosing_function_hash: Some(
            "b9ee44d39137ad72fb72086d588215dc00a601ffc9c606f705230b76bb43a501".to_string(),
        ),
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
            enclosing_function_hash: Some(
                "0105467f2befa106a0483ca9846392a422c7ccb70cdaf93f57d6ba942c4a6b06".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("A"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("func"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("B")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "dfae5a6e06f1ca7eab19bf799456712be278ff21ff66c640fcc49e1cc3a8d52a".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("B"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("C"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("D")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "2a93b3636ca0ff3b3009419fd1918de34c39985afdd44a06d100fe06f9bbf2fc".to_string(),
            ),

            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("C"),
            arguments: vec![],
            enclosing_function_name: Some(s!("D")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "2a93b3636ca0ff3b3009419fd1918de34c39985afdd44a06d100fe06f9bbf2fc".to_string(),
            ),
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
            enclosing_function_hash: Some(s!(
                "9effd4ed97bd589bbe40b7bfc75aa851617dd39b94519b9c2cecbd23c1b0b2f2"
            )),
            is_self_invoke: true,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("a"),
                    value: s!("self.a"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("b"),
                    value: s!("self.b"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Divider")),
            enclosing_function_hash: Some(s!(
                "9effd4ed97bd589bbe40b7bfc75aa851617dd39b94519b9c2cecbd23c1b0b2f2"
            )),
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
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "3051f8e5edfaa306f5bce5b837bdb31bff1ee85083d0b9ec883a4426bd038827"
            )),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("divider.divide"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "3051f8e5edfaa306f5bce5b837bdb31bff1ee85083d0b9ec883a4426bd038827"
            )),

            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),

                    value: s!("self.a"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("sum")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "1f67e43697d4c9cb1d37345ef1ecc13d787c39e4ba41c0c9c7c3ca6553a8aef6"
            )),

            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("classes.sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),

                    value: s!("5"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("4"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("product")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "e980235a22a2fb8369a5aadd7ff30ac9b0abc177c1e2a79bc9f1274c5c39c708"
            )),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("product"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("product")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "e980235a22a2fb8369a5aadd7ff30ac9b0abc177c1e2a79bc9f1274c5c39c708"
            )),

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
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("ValueError"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("f\"User with email {email} already exists\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("User"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("id"),
                    value: s!("len(self.repository.get_all()) +\n                        1"),
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("name"),
                    value: s!("name"),

                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!("email"),
                    value: s!("email"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            enclosing_class_name: Some(s!("UserService")),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("len"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("self.repository.get_all"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.save"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("new_user"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "d47fc56dcd3bc80d405b6c6f7849f935a4d391315f9a794ae4d2e3772801a494".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_by_id"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("get_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "ca0c46aef3a576f8ef8a2c558375f9bc80fad27bd820d155e93afd7c1fa00f02".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "1cdb523c32bb2846c422d25aea92f6cd5b4b6ac5ed764be264b6fb0ffb40002f".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.delete"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("delete_user")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "881e86909cf4775caa588df9cc4badac67e4aee598662f0aff9c8235e1c97ae2".to_string(),
            ),
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
                    datatype: s!("any"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("email"),
                    datatype: s!("any"),
                },
            ],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "54854cdbf1d75b0f306e99e6b085841fcc6def343d2d8f3bf06113aba9b0fe03"
            )),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("str"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("e"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("create_user")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "54854cdbf1d75b0f306e99e6b085841fcc6def343d2d8f3bf06113aba9b0fe03"
            )),

            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.service.get_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("get_user")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "e5253c9c73a4b256a0bdf11e4c71ef596784676d634b68b67bec834f5822e782"
            )),

            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.list_users"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "f7ff461f1cd138ae9ad319bfa32b3bf519eee40956d8871ac1c75b870bf1ec31"
            )),

            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.delete_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("delete_user")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "973497d422ce6d9be30e19ce4fd5057660a82b1f94e187426abafed6d40b50e9"
            )),

            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
    ];

    assert_eq!(calls, expected);
}
