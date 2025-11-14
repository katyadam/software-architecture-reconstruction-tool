use models::{Callable, Namespace, Parameter};
use python_extractor::{
    extraction::{
        callables::extractor::CallablesExtractor,
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
            hash: s!("d74ff0ee8da3b9806b18c877dbf29bbde50b5bd8e4dad7a3a725000feb82e8f1"),
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
            hash: s!("b9ee44d39137ad72fb72086d588215dc00a601ffc9c606f705230b76bb43a501"),
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
            hash: s!("0105467f2befa106a0483ca9846392a422c7ccb70cdaf93f57d6ba942c4a6b06"),
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
            hash: s!("dfae5a6e06f1ca7eab19bf799456712be278ff21ff66c640fcc49e1cc3a8d52a"),
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
            hash: s!("540334dc3979994b09946ffd275c1060a1de79575a84af03c8950cdca9d19bcb"),
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
            hash: s!("2a93b3636ca0ff3b3009419fd1918de34c39985afdd44a06d100fe06f9bbf2fc"),
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
            hash: s!("112eaad45fe3a03aa8d62a93b7e059d7af8a0c63a105fa292666ea46bbe555f1"),
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
            hash: s!("9effd4ed97bd589bbe40b7bfc75aa851617dd39b94519b9c2cecbd23c1b0b2f2"),
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
            hash: s!("f0c5f05ef4e51d6fcf5966c194309b7ae2f087ec45e8eac2f9492656cd1ff08e"),
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
            hash: s!("e148c439e5e3717869a9931caccd290594743bff8ff5b314ffb3b89a6b43b005"),
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
            hash: s!("112eaad45fe3a03aa8d62a93b7e059d7af8a0c63a105fa292666ea46bbe555f1"),
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
            hash: s!("3051f8e5edfaa306f5bce5b837bdb31bff1ee85083d0b9ec883a4426bd038827"),
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
            hash: s!("1f67e43697d4c9cb1d37345ef1ecc13d787c39e4ba41c0c9c7c3ca6553a8aef6"),
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
            hash: s!("e980235a22a2fb8369a5aadd7ff30ac9b0abc177c1e2a79bc9f1274c5c39c708"),
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
            hash: s!("978bab3821eb5935a5c1d5f577d8d1bb49177c1fc6a55aac3c0cb42c2b2c456d"),
            file_path: s!("./examples/python/callgraph/classes-imports.py"),
        },
    ];

    assert_eq!(callables, expected);
}
