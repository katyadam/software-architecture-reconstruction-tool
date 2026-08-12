use models::{Argument, CallStatement};
use python_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::{
            PythonCallStatement, evaluator::evaluate_invocations_on_statements,
            extractor::CallsExtractor,
        },
        extractor::{ExtractParams, Extractor},
    },
    s,
};

use crate::python::utils::{get_tree, load_file, parse_file};

#[test]
fn simple_test() {
    let filename = "./examples/python/callgraph/simple.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));

    let expected = vec![CallStatement {
        function_name: s!("A"),
        arguments: vec![],
        enclosing_function_name: Some(s!("B() -> Any")),
        enclosing_class_name: None,
        enclosing_function_hash: Some(
            "5380b70e23765bad5354b9ebe00d02ff832d744cebd97bffaa2dd2158a24d4fd".to_string(),
        ),
        is_self_invoke: false,
        is_super_invoke: false,
        invoked_on: None,
        is_decorator: false,
    }];
    assert_eq!(
        calls
            .into_iter()
            .map(PythonCallStatement::to_language_agnostic)
            .collect::<Vec<CallStatement>>(),
        expected
    );
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
            enclosing_function_name: Some(s!("A(func) -> Any")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "650fec183ca7b316f2eea955199ded5434c4c0e2519855e71ec4ebc25c52a727".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("A"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("func"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("B(func) -> Any")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "cc712a7d1633c1d66e5b6c092582f40391e2fa9f24fedcb2ec8bbf1f366e84c0".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("B"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("C()"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("D() -> Any")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "745cbe2ba4c4ec3bb0ce4671266169404aadc272d08171daf73fe0648d923159".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("C"),
            arguments: vec![],
            enclosing_function_name: Some(s!("D() -> Any")),
            enclosing_class_name: None,
            enclosing_function_hash: Some(
                "745cbe2ba4c4ec3bb0ce4671266169404aadc272d08171daf73fe0648d923159".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
    ];

    assert_eq!(
        calls
            .into_iter()
            .map(PythonCallStatement::to_language_agnostic)
            .collect::<Vec<CallStatement>>(),
        expected
    );
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
            enclosing_function_name: Some(s!("divide(self) -> float")),
            enclosing_class_name: Some(s!("Divider")),
            enclosing_function_hash: Some(s!(
                "30372f7a99122dc570c1067673de63bdaa1771f42ede99fb45fa3b2f9f1f7dff"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("a"),
                    value: s!("self.a"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!("b"),
                    value: s!("self.b"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("divide(self) -> float")),
            enclosing_class_name: Some(s!("Divider")),
            enclosing_function_hash: Some(s!(
                "30372f7a99122dc570c1067673de63bdaa1771f42ede99fb45fa3b2f9f1f7dff"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
    ];

    assert_eq!(
        calls
            .into_iter()
            .map(PythonCallStatement::to_language_agnostic)
            .collect::<Vec<CallStatement>>(),
        expected
    );
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
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("divide(self) -> Any")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "c8d75a476ca34490e210d69d820334e34f1d6b7ba3e072349fab722550bf0f02"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("divider.divide"),
            arguments: vec![],
            enclosing_function_name: Some(s!("divide(self) -> Any")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "c8d75a476ca34490e210d69d820334e34f1d6b7ba3e072349fab722550bf0f02"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("sum(self) -> Any")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "e3281ab38386c5755a1cdc5868b282d087b75a346aebc95d3f9f602bc463ee07"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("classes.sum"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("5"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("4"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("product(self) -> Any")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "0a9198d819b5cf2e24494807db9cc7baf9ce8355f7e16b4a53e2c97634abf16a"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("product"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.a"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("self.b"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("product(self) -> Any")),
            enclosing_class_name: Some(s!("Math")),
            enclosing_function_hash: Some(s!(
                "0a9198d819b5cf2e24494807db9cc7baf9ce8355f7e16b4a53e2c97634abf16a"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
    ];

    assert_eq!(
        calls
            .into_iter()
            .map(PythonCallStatement::to_language_agnostic)
            .collect::<Vec<CallStatement>>(),
        expected
    );
}

#[test]
fn should_assign_correct_invoke_on_using_assignment_type_inference() {
    let filename = s!("./examples/python/callgraph/repository-pattern/service.py");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let assignments_map = get_assignments_map(&tree, &code);
    let mut agnostic: Vec<CallStatement> = calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect();
    evaluate_invocations_on_statements(&mut agnostic, &assignments_map);

    let expected = vec![
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("ValueError"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("f\"User with email {email} already exists\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("User"),
            arguments: vec![
                Argument {
                    assigned_variable: s!("id"),
                    value: s!("len(self.repository.get_all()) +\n                        1"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!("name"),
                    value: s!("name"),
                    datatype: Some(s!("str")),
                },
                Argument {
                    assigned_variable: s!("email"),
                    value: s!("email"),
                    datatype: Some(s!("str")),
                },
            ],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("len"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("self.repository.get_all()"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.repository.save"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("new_user"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> User")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "973ab5f612126e2b275dbdcffeb78b849ac1d80624c1b9e6c2c3f3c249feb659".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.repository.get_by_id"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: Some(s!("int")),
            }],
            enclosing_function_name: Some(s!("get_user(self, user_id: int) -> Optional[User]")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "c9aa53c61d725d28b93b48855f147cc970abcfa2ad5bbfc1aad57e9f3bd808bf".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.repository.get_all"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users(self) -> List[User]")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "012751b797566799f496a5a4cea681f57965e1cf9555a185c1fb936646280709".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.repository.delete"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: Some(s!("int")),
            }],
            enclosing_function_name: Some(s!("delete_user(self, user_id: int) -> bool")),
            enclosing_class_name: Some(s!("UserService")),
            enclosing_function_hash: Some(
                "1b942e983af8fc70dee0915858abadc7a7aaf421574446d2e6641bc29c764c07".to_string(),
            ),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserRepository")),
            is_decorator: false,
        },
    ];

    assert_eq!(agnostic, expected);
}

#[test]
fn should_assign_correct_invoke_on_using_function_and_assignment_type_inference() {
    let filename = s!("./examples/python/callgraph/repository-pattern/controller.py");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let assignments_map = get_assignments_map(&tree, &code);
    let mut agnostic: Vec<CallStatement> = calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect();
    evaluate_invocations_on_statements(&mut agnostic, &assignments_map);

    let expected = vec![
        CallStatement {
            function_name: s!("self.service.create_user"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("name"),
                    datatype: Some(s!("str")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("email"),
                    datatype: Some(s!("str")),
                },
            ],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> Any")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "ed5478fee2f3c781fc3051f9aa2554533da8e5bbb8e24e0bab95c6e9d39ae0a6"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserService")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("str"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("e"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("create_user(self, name: str, email: str) -> Any")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "ed5478fee2f3c781fc3051f9aa2554533da8e5bbb8e24e0bab95c6e9d39ae0a6"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.service.get_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: Some(s!("int")),
            }],
            enclosing_function_name: Some(s!("get_user(self, user_id: int) -> Any")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "0414661bd8173a6aa11de471a128f2d1ab5bbd8b656959a97ecbecd013333b2c"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserService")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.service.list_users"),
            arguments: vec![],
            enclosing_function_name: Some(s!("list_users(self) -> Any")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "5c801ce913bd39ae28d8151f369868354c8679df1c0f7755b5b1642c9f0a1709"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserService")),
            is_decorator: false,
        },
        CallStatement {
            function_name: s!("self.service.delete_user"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("user_id"),
                datatype: Some(s!("int")),
            }],
            enclosing_function_name: Some(s!("delete_user(self, user_id: int) -> Any")),
            enclosing_class_name: Some(s!("UserController")),
            enclosing_function_hash: Some(s!(
                "1043c2ca95f147c9f1e4abd32db10e679bf3f9941c113df986bbd79c93bbc383"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: Some(s!("UserService")),
            is_decorator: false,
        },
    ];

    assert_eq!(agnostic, expected);
}

/// Verifies that type inference resolves argument datatypes from typed function parameters.
/// `B(a: int)` calls `A(a, c)` where `a` is typed and `c` is an untyped local assignment.
/// After evaluation, `a` must carry `datatype: "int"` and `c` must carry `datatype: "any"`.
#[test]
fn typed_params_infer_argument_datatypes_test() {
    let filename = "./examples/python/callgraph/simple_with_types.py";
    let (code, tree) = parse_file(filename);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let assignments_map = get_assignments_map(&tree, &code);
    let mut agnostic: Vec<CallStatement> = calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect();
    evaluate_invocations_on_statements(&mut agnostic, &assignments_map);

    // B calls A — that is the only call statement in this file
    assert_eq!(agnostic.len(), 1, "expected exactly one call statement");
    let call = &agnostic[0];
    assert_eq!(call.function_name, s!("A"));
    assert_eq!(
        call.enclosing_function_name.as_deref(),
        Some("B(a: int) -> Any"),
        "call should be inside B"
    );

    // argument `a` has type annotation `int` on B's parameter -> should be resolved
    let arg_a = call
        .arguments
        .iter()
        .find(|a| a.value == "a")
        .expect("argument a not found");
    assert_eq!(
        arg_a.datatype,
        Some(s!("int")),
        "typed parameter `a: int` should produce datatype 'int'"
    );

    // argument `c` is a plain literal assignment (`c = 5`) — no type info available
    let arg_c = call
        .arguments
        .iter()
        .find(|a| a.value == "c")
        .expect("argument c not found");
    assert_eq!(
        arg_c.datatype, None,
        "untyped local variable `c` should produce datatype None"
    );
}

#[test]
fn test_call_edge_cases() {
    let filename = "./examples/python/callgraph/call_edge_cases.py";
    let (code, tree) = parse_file(filename);
    let raw_calls = CallsExtractor.extract(ExtractParams::new(&tree, &code));
    let calls: Vec<CallStatement> = raw_calls
        .into_iter()
        .map(PythonCallStatement::to_language_agnostic)
        .collect();

    // Edge case 1: Starred arguments — target(*args, **kwargs) is captured;
    // starred expressions appear as argument values with their raw text
    let delegate_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| {
            c.enclosing_function_name.as_deref() == Some("delegate(target, *args, **kwargs) -> Any")
        })
        .collect();
    assert!(
        !delegate_calls.is_empty(),
        "delegate() should contain at least one call"
    );
    let target_call = delegate_calls
        .iter()
        .find(|c| c.function_name == "target")
        .unwrap();
    assert!(
        target_call
            .arguments
            .iter()
            .any(|a| a.value.contains("args")),
        "starred *args should appear in call arguments"
    );
    assert!(
        target_call
            .arguments
            .iter()
            .any(|a| a.value.contains("kwargs")),
        "starred **kwargs should appear in call arguments"
    );

    // Edge case 2: Call inside a list comprehension — str(x) inside [str(x) for x in items]
    // is still captured as a regular CallStatement
    let transform_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.enclosing_function_name.as_deref() == Some("transform_all(items) -> Any"))
        .collect();
    assert!(
        !transform_calls.is_empty(),
        "str(x) inside list comprehension should be captured"
    );
    assert!(
        transform_calls.iter().any(|c| c.function_name == "str"),
        "str() call inside comprehension must be captured"
    );

    // Edge case 3: isinstance() built-in call — captured as a regular CallStatement
    // with two arguments (the value and the type)
    let handle_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.enclosing_function_name.as_deref() == Some("handle_input(data: Any) -> str"))
        .collect();
    let isinstance_call = handle_calls
        .iter()
        .find(|c| c.function_name == "isinstance")
        .unwrap();
    assert_eq!(
        isinstance_call.arguments.len(),
        2,
        "isinstance takes 2 arguments"
    );
    assert_eq!(isinstance_call.arguments[0].value, s!("data"));
    assert_eq!(isinstance_call.arguments[1].value, s!("str"));
    assert!(
        !isinstance_call.is_self_invoke,
        "isinstance is not a self-invoke"
    );
}

#[test]
fn flags_calls_inside_decorators() {
    let code = r#"
@app.get("/items")
def read_items():
    return requests.get("http://inventory/data")
"#;
    let tree = get_tree(code);
    let calls = CallsExtractor.extract(ExtractParams::new(&tree, code));

    let decorator_call = calls
        .iter()
        .find(|c| c.call_statement.function_name.starts_with("app.get"))
        .expect("app.get(...) inside the decorator must be extracted");
    assert!(
        decorator_call.call_statement.is_decorator,
        "a call whose parent node is a decorator must be flagged"
    );

    let outbound = calls
        .iter()
        .find(|c| c.call_statement.function_name.starts_with("requests.get"))
        .expect("requests.get(...) in the body must be extracted");
    assert!(
        !outbound.call_statement.is_decorator,
        "a call in the function body must not be flagged"
    );
}
