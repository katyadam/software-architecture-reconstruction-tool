use models::{Callable, Namespace, Parameter};
use python_extractor::{
    extraction::{
        callables::extractor::CallablesExtractor,
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
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));
    let expected = vec![
        Callable {
            name: s!("A()"),
            signature: s!("module:./examples/python/callgraph/simple.py/A()"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/simple.py")),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("d9df9ce075e17ae930f1a91628d1d74d5c21fc1c257ebf225d207620d6e6bc66"),
            file_path: s!("./examples/python/callgraph/simple.py"),
        },
        Callable {
            name: s!("B()"),
            signature: s!("module:./examples/python/callgraph/simple.py/B()"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/simple.py")),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("5380b70e23765bad5354b9ebe00d02ff832d744cebd97bffaa2dd2158a24d4fd"),
            file_path: s!("./examples/python/callgraph/simple.py"),
        },
    ];

    assert_eq!(callables, expected);
}

#[test]
fn nested_test() {
    let filename = "./examples/python/callgraph/nested.py";
    let code = load_file(filename).unwrap();
    let tree = get_tree(&code);
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    let expected = vec![
        Callable {
            name: s!("A(func)"),
            signature: s!("module:./examples/python/callgraph/nested.py/A(func)"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/nested.py")),
            parameters: vec![Parameter {
                name: s!("func"),
                datatype: None,
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("650fec183ca7b316f2eea955199ded5434c4c0e2519855e71ec4ebc25c52a727"),
            file_path: s!("./examples/python/callgraph/nested.py"),
        },
        Callable {
            name: s!("B(func)"),
            signature: s!("module:./examples/python/callgraph/nested.py/B(func)"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/nested.py")),
            parameters: vec![Parameter {
                name: s!("func"),
                datatype: None,
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("cc712a7d1633c1d66e5b6c092582f40391e2fa9f24fedcb2ec8bbf1f366e84c0"),
            file_path: s!("./examples/python/callgraph/nested.py"),
        },
        Callable {
            name: s!("C()"),
            signature: s!("module:./examples/python/callgraph/nested.py/C()"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/nested.py")),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("9f978d4dd5fba836729017fe06d71e96a2a8ec3e4a4aacc8bbd06ab9d5a62935"),
            file_path: s!("./examples/python/callgraph/nested.py"),
        },
        Callable {
            name: s!("D()"),
            signature: s!("module:./examples/python/callgraph/nested.py/D()"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/nested.py")),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("745cbe2ba4c4ec3bb0ce4671266169404aadc272d08171daf73fe0648d923159"),
            file_path: s!("./examples/python/callgraph/nested.py"),
        },
    ];

    assert_eq!(callables, expected);
}

#[test]
fn classes_test() {
    let filename = "./examples/python/callgraph/classes.py";
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    let expected = vec![
        Callable {
            name: s!("__init__(self, a:int, b:int)"),
            signature: s!("class:Divider/__init__(self, a:int, b:int)"),
            namespace: Namespace::Class(s!("Divider")),
            parameters: vec![
                Parameter {
                    name: s!("self"),
                    datatype: None,
                    initial_value: None,
                },
                Parameter {
                    name: s!("a"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("b"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
            ],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("c80beec77d695a28bc7e6d631bec5b41344f851a8bc2ebdae2c5ada96cfeb0f8"),
            file_path: s!("./examples/python/callgraph/classes.py"),
        },
        Callable {
            name: s!("divide(self)"),
            signature: s!("class:Divider/divide(self)"),
            namespace: Namespace::Class(s!("Divider")),
            parameters: vec![Parameter {
                name: s!("self"),
                datatype: None,
                initial_value: None,
            }],
            return_type: Some(s!("float")),
            is_async: false,
            is_constructor: false,
            hash: s!("30372f7a99122dc570c1067673de63bdaa1771f42ede99fb45fa3b2f9f1f7dff"),
            file_path: s!("./examples/python/callgraph/classes.py"),
        },
        Callable {
            name: s!("dividable(self)"),
            signature: s!("class:Divider/dividable(self)"),
            namespace: Namespace::Class(s!("Divider")),
            parameters: vec![Parameter {
                name: s!("self"),
                datatype: None,
                initial_value: None,
            }],
            return_type: Some(s!("bool")),
            is_async: false,
            is_constructor: false,
            hash: s!("15e4e25540b3cf1685ffa5be42b9abc0826887109692cca4df61641f3338feef"),
            file_path: s!("./examples/python/callgraph/classes.py"),
        },
        Callable {
            name: s!("sum(a:int, b:int)"),
            signature: s!("module:./examples/python/callgraph/classes.py/sum(a:int, b:int)"),
            namespace: Namespace::Module(s!("./examples/python/callgraph/classes.py")),
            parameters: vec![
                Parameter {
                    name: s!("a"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("b"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("int")),
            is_async: false,
            is_constructor: false,
            hash: s!("0c826868c65d7c2163c167ce67a44f69f3df6284e3f43e43781a20cc2811fed3"),
            file_path: s!("./examples/python/callgraph/classes.py"),
        },
    ];

    assert_eq!(callables, expected);
}

#[test]
fn classes_imports_test() {
    let filename = "./examples/python/callgraph/classes-imports.py";
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    let expected = vec![
        Callable {
            name: s!("__init__(self, a:int, b:int)"),
            signature: s!("class:Math/__init__(self, a:int, b:int)"),
            namespace: Namespace::Class(s!("Math")),
            parameters: vec![
                Parameter {
                    name: s!("self"),
                    datatype: None,
                    initial_value: None,
                },
                Parameter {
                    name: s!("a"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("b"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
            ],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("c80beec77d695a28bc7e6d631bec5b41344f851a8bc2ebdae2c5ada96cfeb0f8"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
        Callable {
            name: s!("divide(self)"),
            signature: s!("class:Math/divide(self)"),
            namespace: Namespace::Class(s!("Math")),
            parameters: vec![Parameter {
                name: s!("self"),
                datatype: None,
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("c8d75a476ca34490e210d69d820334e34f1d6b7ba3e072349fab722550bf0f02"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
        Callable {
            name: s!("sum(self)"),
            signature: s!("class:Math/sum(self)"),
            namespace: Namespace::Class(s!("Math")),
            parameters: vec![Parameter {
                name: s!("self"),
                datatype: None,
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("e3281ab38386c5755a1cdc5868b282d087b75a346aebc95d3f9f602bc463ee07"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
        Callable {
            name: s!("product(self)"),
            signature: s!("class:Math/product(self)"),
            namespace: Namespace::Class(s!("Math")),
            parameters: vec![Parameter {
                name: s!("self"),
                datatype: None,
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: s!("0a9198d819b5cf2e24494807db9cc7baf9ce8355f7e16b4a53e2c97634abf16a"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
        Callable {
            name: s!("product(a:int, b:int)"),
            signature: s!(
                "module:./examples/python/callgraph/classes-imports.py/product(a:int, b:int)"
            ),
            namespace: Namespace::Module(s!("./examples/python/callgraph/classes-imports.py")),
            parameters: vec![
                Parameter {
                    name: s!("a"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("b"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("int")),
            is_async: false,
            is_constructor: false,
            hash: s!("797e6ea6f7b3cba53e2a766831836ec6e973ba26a96d2c5ac11a10db3db92044"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
    ];

    assert_eq!(callables, expected);
}

/// Verifies that typed parameters (`x: int`) are extracted with their datatype,
/// and that untyped parameters have `datatype: None`.
#[test]
fn typed_parameters_test() {
    let filename = "./examples/python/callgraph/simple_with_types.py";
    let (code, tree) = parse_file(filename);
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    assert_eq!(callables.len(), 2, "expected 2 callables (A and B)");

    let a = callables.iter().find(|c| c.name.starts_with("A(")).expect("callable A not found");
    assert_eq!(a.parameters.len(), 2);
    assert_eq!(a.parameters[0].name, s!("x"));
    assert_eq!(a.parameters[0].datatype, Some(s!("int")));
    assert_eq!(a.parameters[1].name, s!("y"));
    assert_eq!(a.parameters[1].datatype, Some(s!("int")));
    assert!(!a.is_async);
    assert!(!a.is_constructor);

    let b = callables.iter().find(|c| c.name.starts_with("B(")).expect("callable B not found");
    assert_eq!(b.parameters.len(), 1);
    assert_eq!(b.parameters[0].name, s!("a"));
    assert_eq!(b.parameters[0].datatype, Some(s!("int")));
    assert!(!b.is_async);
}

/// Verifies that the `is_async` flag is correctly extracted for both `async def` and
/// regular `def` functions in a complex real-world file with nested control flow.
#[test]
fn async_functions_test() {
    let filename = "./examples/python/callgraph/cycle.py";
    let (code, tree) = parse_file(filename);
    let callables =
        CallablesExtractor.extract(ExtractParams::new(&tree, &code).file_name(&s!(filename)));

    let async_names: Vec<&str> = callables
        .iter()
        .filter(|c| c.is_async)
        .map(|c| c.name.as_str())
        .collect();
    let sync_names: Vec<&str> = callables
        .iter()
        .filter(|c| !c.is_async)
        .map(|c| c.name.as_str())
        .collect();

    // All `async def` functions must be detected as async
    for expected in &[
        "schedule_ready_jobs()",
        "_fetch_jobs(statuses)",
        "_send_ready_jobs_to_jes(jobs)",
        "_update_state_for_jobs_not_in_ready_state(jobs)",
        "_update_job_status(job_id, status, error_message)",
        "_get_execution(job_id)",
        "_fetch_raw_token(job)",
        "_create_execution_request_raw(job, access_token, timeout=-1)",
    ] {
        assert!(
            async_names.iter().any(|n| n.starts_with(expected)),
            "expected async callable starting with '{expected}', got: {async_names:?}"
        );
    }

    // Regular `def` functions must NOT be marked async
    for expected in &[
        "_http_exception_log(e)",
        "_log_exception_info(e, additional_message)",
        "_jobs_ready(jobs)",
        "_jobs_not_ready(jobs)",
    ] {
        assert!(
            sync_names.iter().any(|n| n.starts_with(expected)),
            "expected sync callable starting with '{expected}', got: {sync_names:?}"
        );
    }
}
