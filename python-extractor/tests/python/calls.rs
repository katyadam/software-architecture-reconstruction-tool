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
        enclosing_function_name: Some(s!("B()")),
        enclosing_class_name: None,
        enclosing_function_hash: Some(
            "5380b70e23765bad5354b9ebe00d02ff832d744cebd97bffaa2dd2158a24d4fd".to_string(),
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
            enclosing_function_name: Some(s!("A(func)")), // add parameters
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "650fec183ca7b316f2eea955199ded5434c4c0e2519855e71ec4ebc25c52a727".to_string(),
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
            enclosing_function_name: Some(s!("B(func)")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "cc712a7d1633c1d66e5b6c092582f40391e2fa9f24fedcb2ec8bbf1f366e84c0".to_string(),
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
            enclosing_function_name: Some(s!("D()")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "745cbe2ba4c4ec3bb0ce4671266169404aadc272d08171daf73fe0648d923159".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("C"),
            arguments: vec![],
            enclosing_function_name: Some(s!("D()")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "745cbe2ba4c4ec3bb0ce4671266169404aadc272d08171daf73fe0648d923159".to_string(),
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
            enclosing_function_name: Some(s!("divide(self)")), // add (self)
            enclosing_class_name: Some(s!("Divider")),
            enclosing_function_hash: Some(s!(
                "30372f7a99122dc570c1067673de63bdaa1771f42ede99fb45fa3b2f9f1f7dff"
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
            enclosing_function_name: Some(s!("divide(self)")), // add (self)
            enclosing_class_name: Some(s!("Divider")),
            enclosing_function_hash: Some(s!(
                "30372f7a99122dc570c1067673de63bdaa1771f42ede99fb45fa3b2f9f1f7dff"
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
            enclosing_function_name: Some(s!("divide(self)")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "c8d75a476ca34490e210d69d820334e34f1d6b7ba3e072349fab722550bf0f02"
            )),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("divider.divide"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide(self)")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "c8d75a476ca34490e210d69d820334e34f1d6b7ba3e072349fab722550bf0f02"
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
            enclosing_function_name: Some(s!("sum(self)")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "e3281ab38386c5755a1cdc5868b282d087b75a346aebc95d3f9f602bc463ee07"
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
            enclosing_function_name: Some(s!("product(self)")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "0a9198d819b5cf2e24494807db9cc7baf9ce8355f7e16b4a53e2c97634abf16a"
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
            enclosing_function_name: Some(s!("product(self)")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "0a9198d819b5cf2e24494807db9cc7baf9ce8355f7e16b4a53e2c97634abf16a"
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
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
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
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
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
                    datatype: s!("str"),
                },
                Argument {
                    assigned_variable: s!("email"),
                    value: s!("email"),
                    datatype: s!("str"),
                },
            ],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
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
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
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
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_by_id"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("int"),
            }],
            enclosing_function_name: Some(s!("get_user(self, user_id: int)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "c9aa53c61d725d28b93b48855f147cc970abcfa2ad5bbfc1aad57e9f3bd808bf".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users(self)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "012751b797566799f496a5a4cea681f57965e1cf9555a185c1fb936646280709".to_string(),
            ),
            is_self_invoke: true,
            invoked_on: Some(s!("UserRepository")),
        },
        CallStatement {
            function_name: s!("self.repository.delete"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("int"),
            }],
            enclosing_function_name: Some(s!("delete_user(self, user_id: int)")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "1b942e983af8fc70dee0915858abadc7a7aaf421574446d2e6641bc29c764c07".to_string(),
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
                    datatype: s!("str"),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("email"),
                    datatype: s!("str"),
                },
            ],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "ed5478fee2f3c781fc3051f9aa2554533da8e5bbb8e24e0bab95c6e9d39ae0a6"
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
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str)")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "ed5478fee2f3c781fc3051f9aa2554533da8e5bbb8e24e0bab95c6e9d39ae0a6"
            )),
            is_self_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("self.service.get_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("int"),
            }],
            enclosing_function_name: Some(s!("get_user(self, user_id: int)")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "0414661bd8173a6aa11de471a128f2d1ab5bbd8b656959a97ecbecd013333b2c"
            )),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.list_users"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users(self)")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "5c801ce913bd39ae28d8151f369868354c8679df1c0f7755b5b1642c9f0a1709"
            )),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
        CallStatement {
            function_name: s!("self.service.delete_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: s!("int"),
            }],
            enclosing_function_name: Some(s!("delete_user(self, user_id: int)")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "1043c2ca95f147c9f1e4abd32db10e679bf3f9941c113df986bbd79c93bbc383"
            )),
            is_self_invoke: true,
            invoked_on: Some(s!("UserService")),
        },
    ];

    assert_eq!(calls, expected);
}
