use java_extractor::{
    extraction::{
        assignments::map::get_assignments_map,
        calls::{evaluator::evaluate_invocations, extractor::CallStatementsExtractor},
        extractor::Extractor,
    },
    s,
};
use models::{Argument, CallStatement, source_code::SourceSpan};

use crate::java::utils::{get_tree, load_file, parse_file};

#[test]
fn test_all_call_statements() {
    let filename = s!("./examples/AllCallStatements.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let calls = CallStatementsExtractor.extract(&code, &tree, &filename);

    let expected = vec![
        CallStatement {
            function_name: s!("CallPossibilities()"),
            arguments: vec![],
            enclosing_function_name: None,
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: None,
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("this(\"Overloaded Call\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Overloaded Call\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("CallPossibilities()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "73d683675f4c6adc7fe448de8184b78d3e3c5e01a295dd7574a1ffac396222fc"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("super()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("CallPossibilities(String msg)")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "70a82fd1120a4b23b4bb2918a8e0fa101e20b9268d76f50078d85d5fb43e1b51"
            )),
            is_self_invoke: false,
            is_super_invoke: true,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("System.out.println(msg)"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("msg"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("CallPossibilities(String msg)")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "70a82fd1120a4b23b4bb2918a8e0fa101e20b9268d76f50078d85d5fb43e1b51"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("this.internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("super.parentMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: true,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("StaticTarget.staticAction()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("r.run()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("printer.accept(\"Method Reference Call\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Method Reference Call\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("listSupplier.get()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("this.getClass()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("this.getClass().getMethod(\"internalMethod\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"internalMethod\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("m.invoke(this)"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("this"),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim().toUpperCase()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim().toUpperCase().concat(\" WORLD\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\" WORLD\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void demonstrateCalls()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("System.out.println(\"Internal method executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Internal method executed.\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void internalMethod()")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "ac5f54dc66b0f87fbebe2203db38eb9e3df24c5ced02f55fd5e86351eb05d7fe"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 134, end_byte: 1850 },
        },
        CallStatement {
            function_name: s!("System.out.println(\"Parent method executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Parent method executed.\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void parentMethod()")),
            enclosing_class_name: Some(s!("ParentClass")),
            enclosing_function_hash: Some(s!(
                "a7f85f945aff0016bd880edddf5f477d66022067c405dbdf14450f5ed96e006c"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 1852, end_byte: 1960 },
        },
        CallStatement {
            function_name: s!("System.out.println(\"Static call executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Static call executed.\""),
                datatype: None,
            }],
            enclosing_function_name: Some(s!("void staticAction()")),
            enclosing_class_name: Some(s!("StaticTarget")),
            enclosing_function_hash: Some(s!(
                "f747b5e7fd8dea17cae85afcc25fe948e2934bb53e0bca837104bff594937507"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 1962, end_byte: 2076 },
        },
    ];

    assert_eq!(calls, expected);
}

#[test]
fn test_call_statements_evaluation_with_method_overloading() {
    let filename = s!("./examples/MethodOverloading.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let mut calls = CallStatementsExtractor.extract(&code, &tree, &filename);
    let assignments_map = get_assignments_map(&tree, &code);
    evaluate_invocations(&mut calls, &assignments_map);

    let expected = [
        CallStatement {
            function_name: s!("add(a, b)"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("a"),
                    datatype: Some(s!("int")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("b"),
                    datatype: Some(s!("int")),
                },
            ],
            enclosing_function_name: Some(s!("void demo()")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "afb2375ed8e435c50ba143b3f60cca2f9526bf937ed8354fe01e0ff3891bf584"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!("add(2.5, 3.5)"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("2.5"),
                    datatype: Some(s!("double")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("3.5"),
                    datatype: Some(s!("double")),
                },
            ],
            enclosing_function_name: Some(s!("void demo()")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "afb2375ed8e435c50ba143b3f60cca2f9526bf937ed8354fe01e0ff3891bf584"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!("add(1, 2, 3)"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("1"),
                    datatype: Some(s!("int")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("2"),
                    datatype: Some(s!("int")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("3"),
                    datatype: Some(s!("int")),
                },
            ],
            enclosing_function_name: Some(s!("void demo()")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "afb2375ed8e435c50ba143b3f60cca2f9526bf937ed8354fe01e0ff3891bf584"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!("add(\"Hello, \", \"World!\")"),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("\"Hello, \""),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("\"World!\""),
                    datatype: Some(s!("String")),
                },
            ],
            enclosing_function_name: Some(s!("void demo()")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "afb2375ed8e435c50ba143b3f60cca2f9526bf937ed8354fe01e0ff3891bf584"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!("Calculator()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void main(String[] args)")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "a3f16cbfdcabd227e01b1f9a919c07f0bc040419f61fd5077c7d2ed1b5169752"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!("calculator.demo()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("void main(String[] args)")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "a3f16cbfdcabd227e01b1f9a919c07f0bc040419f61fd5077c7d2ed1b5169752"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: Some(s!("Calculator")),
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
        CallStatement {
            function_name: s!(
                "restTemplate.exchange( order_service_url + \"/api/v1/orderservice/order\", HttpMethod.PUT, requestEntity, Response.class)"
            ),
            arguments: vec![
                Argument {
                    assigned_variable: s!(""),
                    value: s!("order_service_url + \"/api/v1/orderservice/order\""),
                    datatype: Some(s!("String")),
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("HttpMethod.PUT"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("requestEntity"),
                    datatype: None,
                },
                Argument {
                    assigned_variable: s!(""),
                    value: s!("Response.class"),
                    datatype: None,
                },
            ],
            enclosing_function_name: Some(s!("void main(String[] args)")),
            enclosing_class_name: Some(s!("Calculator")),
            enclosing_function_hash: Some(s!(
                "a3f16cbfdcabd227e01b1f9a919c07f0bc040419f61fd5077c7d2ed1b5169752"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: Some(s!("RestTemplate")),
            source_span: SourceSpan { start_byte: 34, end_byte: 971 },
        },
    ];

    assert_eq!(calls, expected);
}

#[test]
fn test_call_statements_user_account() {
    let filename = s!("./examples/UserAccount.java");
    let (code, tree) = parse_file(&filename);
    let calls = CallStatementsExtractor.extract(&code, &tree, &filename);

    // UserAccount has 3 method calls: 2× Objects.equals (in equals()) and 1× Objects.hash (in hashCode())
    // this.active = false is an assignment, not a call — must not appear
    assert_eq!(
        calls.len(),
        3,
        "Expected 3 calls: 2× Objects.equals + 1× Objects.hash"
    );

    let equals_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name.contains("Objects.equals"))
        .collect();
    assert_eq!(equals_calls.len(), 2, "Expected 2 Objects.equals calls");
    assert!(
        equals_calls
            .iter()
            .all(|c| c.enclosing_function_name.as_deref() == Some("boolean equals(Object o)")),
        "Both Objects.equals calls should be inside equals()"
    );

    let hash_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name.contains("Objects.hash"))
        .collect();
    assert_eq!(hash_calls.len(), 1, "Expected 1 Objects.hash call");
    assert_eq!(
        hash_calls[0].enclosing_function_name.as_deref(),
        Some("int hashCode()"),
        "Objects.hash should be inside hashCode()"
    );

    // Verify that the assignment this.active = false is NOT extracted as a call
    assert!(
        calls
            .iter()
            .all(|c| !c.function_name.starts_with("this.active")),
        "this.active = false is an assignment, not a call"
    );
}

#[test]
fn test_call_statement_edge_cases() {
    let filename = s!("./examples/CallStatementEdgeCases.java");
    let (code, tree) = parse_file(&filename);
    let calls = CallStatementsExtractor.extract(&code, &tree, &filename);

    // Edge case 1: Chained calls — input.trim().toLowerCase() produces two separate CallStatements:
    //   the inner call "input.trim()" and the outer "input.trim().toLowerCase()"
    let trim_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name == s!("input.trim()"))
        .collect();
    assert_eq!(
        trim_calls.len(),
        1,
        "input.trim() should be captured as an independent call"
    );
    assert_eq!(
        trim_calls[0].enclosing_function_name.as_deref(),
        Some("String normalize(String input)")
    );

    let chained_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name == s!("input.trim().toLowerCase()"))
        .collect();
    assert_eq!(
        chained_calls.len(),
        1,
        "The full chain input.trim().toLowerCase() is the outer call's function_name"
    );
    assert_eq!(
        chained_calls[0].enclosing_function_name.as_deref(),
        Some("String normalize(String input)")
    );

    // Edge case 2: Object creation — new StringBuilder(content) is captured via
    // object_creation_expression, not method_invocation. function_name = "StringBuilder(content)".
    let obj_creation: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name.starts_with("StringBuilder"))
        .collect();
    assert_eq!(
        obj_creation.len(),
        1,
        "new StringBuilder(content) should be captured as a call"
    );
    assert_eq!(
        obj_creation[0].enclosing_function_name.as_deref(),
        Some("String buildMessage(String content)")
    );

    // Edge case 3: Call inside if-condition — s.isEmpty() is inside a boolean guard,
    // not a statement-level expression, but is still captured.
    let guard_calls: Vec<&CallStatement> = calls
        .iter()
        .filter(|c| c.function_name == s!("s.isEmpty()"))
        .collect();
    assert_eq!(
        guard_calls.len(),
        1,
        "s.isEmpty() inside if() must still be captured"
    );
    assert_eq!(
        guard_calls[0].enclosing_function_name.as_deref(),
        Some("boolean isBlank(String s)")
    );
}
