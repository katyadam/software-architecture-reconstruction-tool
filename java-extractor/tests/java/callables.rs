use java_extractor::{
    extraction::{callables::extractor::CallablesExtractor, extractor::Extractor},
    s,
};
use models::{Callable, Namespace, Parameter};

use crate::java::utils::{get_tree, load_file, parse_file};

#[test]
fn test_callables() {
    let filename = s!("./examples/AllMethods.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let callables = CallablesExtractor.extract(&code, &tree, &filename);

    let expected = vec![
        Callable {
            name: s!("MethodHeadersDemo()"),
            signature: s!("class:MethodHeadersDemo/MethodHeadersDemo()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("67b044353cddbba3fe2805792ca913f899f2eca681c1a72e0b1f38d769ade3c5"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("MethodHeadersDemo(T value)"),
            signature: s!("class:MethodHeadersDemo/MethodHeadersDemo(T value)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("value"),
                datatype: Some(s!("T")),
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("8315ec925b28ada9cfefee0729f98fb62e3ec618798414881b468b3c768091f7"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("MethodHeadersDemo(int x, String y)"),
            signature: s!("class:MethodHeadersDemo/MethodHeadersDemo(int x, String y)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![
                Parameter {
                    name: s!("x"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("y"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
            ],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("09bd3f7ac05f5602018e7a85e289344acd07c1e5f6b15debf90a7adbb2368d9f"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("MethodHeadersDemo(double hidden)"),
            signature: s!("class:MethodHeadersDemo/MethodHeadersDemo(double hidden)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("hidden"),
                datatype: Some(s!("double")),
                initial_value: None,
            }],
            return_type: None,
            is_async: false,
            is_constructor: true,
            hash: s!("e64deaee36e753ee285ef730f9c8044651a7d9e95f8ee5c0caec56e49b984c97"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void publicMethod()"),
            signature: s!("class:MethodHeadersDemo/void publicMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("b1240cb6fc674ab9b848cc4d0362c97232a837fccc2d87eb3ee27e3f934bf5c2"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("String protectedMethod(int a, long b)"),
            signature: s!("class:MethodHeadersDemo/String protectedMethod(int a, long b)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![
                Parameter {
                    name: s!("a"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("b"),
                    datatype: Some(s!("long")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("String")),
            is_async: false,
            is_constructor: false,
            hash: s!("904436a68d19a7984f4d31547401f2afabbefe2f44bf086e974ab30e1ce464f0"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void packagePrivateMethod(List<String> list)"),
            signature: s!("class:MethodHeadersDemo/void packagePrivateMethod(List<String> list)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("list"),
                datatype: Some(s!("List<String>")),
                initial_value: None,
            }],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("66c97f7c2a6dac2bcdb5251628643bc13413c1b50eb3e6e1f3ab92edf0dfb6ec"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("int privateMethod(Map<String, Integer> map)"),
            signature: s!("class:MethodHeadersDemo/int privateMethod(Map<String, Integer> map)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("map"),
                datatype: Some(s!("Map<String, Integer>")),
                initial_value: None,
            }],
            return_type: Some(s!("int")),
            is_async: false,
            is_constructor: false,
            hash: s!("9bfc958295f8e2dca7e2136e37ff49daeda03e0c3ec0021481778f4f814ebc50"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("boolean finalMethod(Set<? extends T> values)"),
            signature: s!("class:MethodHeadersDemo/boolean finalMethod(Set<? extends T> values)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("values"),
                datatype: Some(s!("Set<? extends T>")),
                initial_value: None,
            }],
            return_type: Some(s!("boolean")),
            is_async: false,
            is_constructor: false,
            hash: s!("3908a2636a405418332c2c7b6063cd7a7eb70d1739d159fe5ace5273f263b054"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void synchronizedMethod()"),
            signature: s!("class:MethodHeadersDemo/void synchronizedMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("be3fc9f8522a053e3cb427c7d247faa09f1b9f652834082c086203c78e9c1ebe"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void nativeMethod()"),
            signature: s!("class:MethodHeadersDemo/void nativeMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("284f35024dadfcc6fe996b355a196b2e98b7e0f62e726a8e47a057b532fa0559"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("T abstractMethod(List<? super T> input)"),
            signature: s!("class:MethodHeadersDemo/T abstractMethod(List<? super T> input)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("input"),
                datatype: Some(s!("List<? super T>")),
                initial_value: None,
            }],
            return_type: Some(s!("T")),
            is_async: false,
            is_constructor: false,
            hash: s!("df02237bf26a841d2f4d37032adfe598813eb2cd65379a72093b3834c391e1d1"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void publicStaticMethod()"),
            signature: s!("class:MethodHeadersDemo/void publicStaticMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("0a002dc56e2022d9073c95e06edb46d0bd7a9d3284d8a476e91eae098108f940"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("int packagePrivateStaticMethod(int[] array)"),
            signature: s!("class:MethodHeadersDemo/int packagePrivateStaticMethod(int[] array)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("array"),
                datatype: Some(s!("int[]")),
                initial_value: None,
            }],
            return_type: Some(s!("int")),
            is_async: false,
            is_constructor: false,
            hash: s!("4a39a1a4780ac3190c093019c36a1fb0cc278354b159ec9530d7ac26f6850aca"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!(
                "Map<String, List<Integer>> privateStaticMethod( Map<String, List<Integer>> input)"
            ),
            signature: s!(
                "class:MethodHeadersDemo/Map<String, List<Integer>> privateStaticMethod( Map<String, List<Integer>> input)"
            ),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("input"),
                datatype: Some(s!("Map<String, List<Integer>>")),
                initial_value: None,
            }],
            return_type: Some(s!("Map<String, List<Integer>>")),
            is_async: false,
            is_constructor: false,
            hash: s!("0c8c30e9e9af1275fed373bf8472e6cfa9bbcba969d9defe41b28187db1f9cbc"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("Map<K, V> genericStaticMethod( List<K> keys, List<V> values)"),
            signature: s!(
                "class:MethodHeadersDemo/Map<K, V> genericStaticMethod( List<K> keys, List<V> values)"
            ),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![
                Parameter {
                    name: s!("keys"),
                    datatype: Some(s!("List<K>")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("values"),
                    datatype: Some(s!("List<V>")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("Map<K, V>")),
            is_async: false,
            is_constructor: false,
            hash: s!("422097f3d3f17005018c5be59d1b8690c6e2e557d26519bb521c6a990ee92b20"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("E genericMethod(E value)"),
            signature: s!("class:MethodHeadersDemo/E genericMethod(E value)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("value"),
                datatype: Some(s!("E")),
                initial_value: None,
            }],
            return_type: Some(s!("E")),
            is_async: false,
            is_constructor: false,
            hash: s!("3e141563a92ef2a33b87e707fdd5550de45b73ec9b7d6d2a8e7c248399d143db"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("List<E> boundedGenericMethod( List<E> list)"),
            signature: s!("class:MethodHeadersDemo/List<E> boundedGenericMethod( List<E> list)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("list"),
                datatype: Some(s!("List<E>")),
                initial_value: None,
            }],
            return_type: Some(s!("List<E>")),
            is_async: false,
            is_constructor: false,
            hash: s!("5efd98911506c3483f4797454bca8dcfc99377d73d5910b3f6d4d8ece168db70"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void throwsGenericException()"),
            signature: s!("class:MethodHeadersDemo/void throwsGenericException()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("8dff460a84af27969c19b0242c142793ce273a8f49b82117b1424e772d4d62dc"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void varArgsMethod(String... args)"),
            signature: s!("class:MethodHeadersDemo/void varArgsMethod(String... args)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("args"),
                datatype: Some(s!("String...")),
                initial_value: None,
            }],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("7445107f4f118305bc68709ce47fc8dc41048d59c6220592895e1d24c406b9b9"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void mixedArrayMethod( int[] ints, List<String>[] listArray)"),
            signature: s!(
                "class:MethodHeadersDemo/void mixedArrayMethod( int[] ints, List<String>[] listArray)"
            ),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![
                Parameter {
                    name: s!("ints"),
                    datatype: Some(s!("int[]")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("listArray"),
                    datatype: Some(s!("List<String>[]")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("4b63d221f0868a83f2b5af8acc1308478533a16d901000f52912e2455e082f9b"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void throwsMethod()"),
            signature: s!("class:MethodHeadersDemo/void throwsMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("aef0f7f27f26f89cd85d7e70a97f98a1f5414be86e379e96a7d221ab95528376"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void throwsMultipleMethod()"),
            signature: s!("class:MethodHeadersDemo/void throwsMultipleMethod()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("24fc7a345e44f719daac116e763fa0dbf532f626c01865a67843b3c5923e93f7"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!(
                "Optional<String> annotatedMethod( @Deprecated List<@Deprecated String> input)"
            ),
            signature: s!(
                "class:MethodHeadersDemo/Optional<String> annotatedMethod( @Deprecated List<@Deprecated String> input)"
            ),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("input"),
                datatype: Some(s!("List<@Deprecated String>")),
                initial_value: None,
            }],
            return_type: Some(s!("Optional<String>")),
            is_async: false,
            is_constructor: false,
            hash: s!("c54c36344517838cd468e9ba70451d3b26208b5aa111f06bdeeea506afb3d767"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void overloaded()"),
            signature: s!("class:MethodHeadersDemo/void overloaded()"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("f43b16264c11c057c870342bde85f43156032697e1aac5256488b177c6161b40"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void overloaded(int x)"),
            signature: s!("class:MethodHeadersDemo/void overloaded(int x)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![Parameter {
                name: s!("x"),
                datatype: Some(s!("int")),
                initial_value: None,
            }],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("1584c0279bdb94f6df0f38d795b8fcd63e89c8fd7450392c53bda0686a73a5d6"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void overloaded(int x, String y)"),
            signature: s!("class:MethodHeadersDemo/void overloaded(int x, String y)"),
            namespace: Namespace::Class(s!("MethodHeadersDemo")),
            parameters: vec![
                Parameter {
                    name: s!("x"),
                    datatype: Some(s!("int")),
                    initial_value: None,
                },
                Parameter {
                    name: s!("y"),
                    datatype: Some(s!("String")),
                    initial_value: None,
                },
            ],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("e624f8a1a347d8e34d0db754c2c2649aac0013e7dcaff9e5aea70e93fa508eb0"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("void interfaceMethod()"),
            signature: s!("class:InnerInterface/void interfaceMethod()"),
            namespace: Namespace::Class(s!("InnerInterface")),
            parameters: vec![],
            return_type: Some(s!("void")),
            is_async: false,
            is_constructor: false,
            hash: s!("8491485dc9ff2f304ba06637e21431f1940067b16dbd8f9bd5adff57fead0941"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("String defaultMethod(List<String> list)"),
            signature: s!("class:InnerInterface/String defaultMethod(List<String> list)"),
            namespace: Namespace::Class(s!("InnerInterface")),
            parameters: vec![Parameter {
                name: s!("list"),
                datatype: Some(s!("List<String>")),
                initial_value: None,
            }],
            return_type: Some(s!("String")),
            is_async: false,
            is_constructor: false,
            hash: s!("e529b8cd66f0725df8b582bff5d5dc16f5952687c2acf3909456717a2075b52c"),
            file_path: s!("./examples/AllMethods.java"),
        },
        Callable {
            name: s!("int staticInterfaceMethod(Map<String, Integer> map)"),
            signature: s!(
                "class:InnerInterface/int staticInterfaceMethod(Map<String, Integer> map)"
            ),
            namespace: Namespace::Class(s!("InnerInterface")),
            parameters: vec![Parameter {
                name: s!("map"),
                datatype: Some(s!("Map<String, Integer>")),
                initial_value: None,
            }],
            return_type: Some(s!("int")),
            is_async: false,
            is_constructor: false,
            hash: s!("eaa3ed9ccccac51fbdd9d3093e2d68f9ec2e972c8ffc0a9f5ebf91f6b92b8d4d"),
            file_path: s!("./examples/AllMethods.java"),
        },
    ];

    assert_eq!(callables, expected);
}

#[test]
fn test_callables_user_account() {
    let filename = s!("./examples/UserAccount.java");
    let (code, tree) = parse_file(&filename);
    let callables = CallablesExtractor.extract(&code, &tree, &filename);

    assert_eq!(callables.len(), 9, "Expected 1 constructor + 8 methods");

    // All callables are in UserAccount class
    assert!(
        callables
            .iter()
            .all(|c| c.namespace == Namespace::Class(s!("UserAccount"))),
        "All callables should be in UserAccount namespace"
    );

    // Exactly 1 constructor
    let constructors: Vec<&Callable> = callables.iter().filter(|c| c.is_constructor).collect();
    assert_eq!(constructors.len(), 1);
    let ctor = constructors[0];
    assert_eq!(
        ctor.name,
        s!("UserAccount(String id, String username, boolean active)")
    );
    assert_eq!(ctor.parameters.len(), 3);
    assert_eq!(ctor.parameters[0].name, s!("id"));
    assert_eq!(ctor.parameters[0].datatype, Some(s!("String")));
    assert_eq!(ctor.parameters[1].name, s!("username"));
    assert_eq!(ctor.parameters[1].datatype, Some(s!("String")));
    assert_eq!(ctor.parameters[2].name, s!("active"));
    assert_eq!(ctor.parameters[2].datatype, Some(s!("boolean")));

    // Check key methods by name
    let method_names: Vec<&str> = callables
        .iter()
        .filter(|c| !c.is_constructor)
        .map(|c| c.name.as_str())
        .collect();
    assert!(method_names.iter().any(|&n| n.contains("getId")));
    assert!(method_names.iter().any(|&n| n.contains("isActive")));
    assert!(method_names.iter().any(|&n| n.contains("deactivate")));
    assert!(method_names.iter().any(|&n| n.contains("changeUsername")));
    assert!(method_names.iter().any(|&n| n.contains("getUsername")));
    assert!(method_names.iter().any(|&n| n.contains("equals")));
    assert!(method_names.iter().any(|&n| n.contains("hashCode")));
    assert!(method_names.iter().any(|&n| n.contains("toString")));

    // equals takes Object parameter
    let equals = callables
        .iter()
        .find(|c| c.name.contains("equals"))
        .unwrap();
    assert_eq!(equals.parameters.len(), 1);
    assert_eq!(equals.parameters[0].name, s!("o"));
    assert_eq!(equals.parameters[0].datatype, Some(s!("Object")));

    // changeUsername takes String parameter
    let change_username = callables
        .iter()
        .find(|c| c.name.contains("changeUsername"))
        .unwrap();
    assert_eq!(change_username.parameters.len(), 1);
    assert_eq!(change_username.parameters[0].name, s!("newName"));
    assert_eq!(change_username.parameters[0].datatype, Some(s!("String")));
}

#[test]
fn test_callable_edge_cases() {
    let filename = s!("./examples/CallableEdgeCases.java");
    let (code, tree) = parse_file(&filename);
    let callables = CallablesExtractor.extract(&code, &tree, &filename);

    // Total: 2 methods in outer class (create, findMax) + 2 in Validator inner class = 4
    assert_eq!(
        callables.len(),
        4,
        "Expected 2 outer + 2 inner class callables"
    );

    // Edge case 1: Annotated parameters — @RequestBody and @PathVariable are stripped from
    // the Parameter fields, but the annotations remain in the callable's name string.
    let create = callables
        .iter()
        .find(|c| c.name.contains("create"))
        .unwrap();
    assert!(
        create.name.contains("@RequestBody"),
        "Annotations remain in callable name"
    );
    assert!(
        create.name.contains("@PathVariable"),
        "Annotations remain in callable name"
    );
    assert_eq!(create.parameters.len(), 2);
    // Annotations are stripped from extracted parameter types
    assert_eq!(create.parameters[0].name, s!("body"));
    assert_eq!(create.parameters[0].datatype, Some(s!("Object")));
    assert_eq!(create.parameters[1].name, s!("id"));
    assert_eq!(create.parameters[1].datatype, Some(s!("String")));

    // Edge case 2: Generic bounded type method — the type bound <T extends Comparable<T>> is
    // not reflected in the callable name; only the bare return type "T" and param type "List<T>" are kept.
    let find_max = callables
        .iter()
        .find(|c| c.name.contains("findMax"))
        .unwrap();
    assert!(
        find_max.name.starts_with("T findMax"),
        "Return type is bare T, not the bound"
    );
    assert!(
        !find_max.name.contains("Comparable"),
        "Type bound is not included in name"
    );
    assert_eq!(find_max.parameters.len(), 1);
    assert_eq!(find_max.parameters[0].datatype, Some(s!("List<T>")));

    // Edge case 3: Inner class methods use the inner class as their namespace
    let validator_callables: Vec<&Callable> = callables
        .iter()
        .filter(|c| c.namespace == Namespace::Class(s!("Validator")))
        .collect();
    assert_eq!(
        validator_callables.len(),
        2,
        "Both Validator methods should have Validator as namespace"
    );
    assert!(
        validator_callables
            .iter()
            .any(|c| c.name.contains("validate"))
    );
    assert!(
        validator_callables
            .iter()
            .any(|c| c.name.contains("report"))
    );

    // Outer class methods have CallableEdgeCases as namespace
    let outer_callables: Vec<&Callable> = callables
        .iter()
        .filter(|c| c.namespace == Namespace::Class(s!("CallableEdgeCases")))
        .collect();
    assert_eq!(outer_callables.len(), 2);
}
