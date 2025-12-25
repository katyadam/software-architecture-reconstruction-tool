use java_extractor::{
    extraction::{calls::extraction::CallStatementsExtractor, extractor::Extractor},
    s,
};
use models::{Argument, CallStatement};

use crate::java::utils::{get_tree, load_file};

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
        },
        CallStatement {
            function_name: s!("this(\"Overloaded Call\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Overloaded Call\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("CallPossibilities")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "73d683675f4c6adc7fe448de8184b78d3e3c5e01a295dd7574a1ffac396222fc"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("super()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("CallPossibilities")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "70a82fd1120a4b23b4bb2918a8e0fa101e20b9268d76f50078d85d5fb43e1b51"
            )),
            is_self_invoke: false,
            is_super_invoke: true,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("System.out.println(msg)"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("msg"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("CallPossibilities")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "70a82fd1120a4b23b4bb2918a8e0fa101e20b9268d76f50078d85d5fb43e1b51"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("this.internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("super.parentMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: true,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("StaticTarget.staticAction()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("internalMethod()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("r.run()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("printer.accept(\"Method Reference Call\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Method Reference Call\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("listSupplier.get()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("this.getClass()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: true,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("this.getClass().getMethod(\"internalMethod\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"internalMethod\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("m.invoke(this)"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("this"),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim().toUpperCase()"),
            arguments: vec![],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("\"  hello  \".trim().toUpperCase().concat(\" WORLD\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\" WORLD\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("demonstrateCalls")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "114af928d1c7371e42f5fb79489cdacac208efb2fb8898ebc010db657f63fce9"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("System.out.println(\"Internal method executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Internal method executed.\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("internalMethod")),
            enclosing_class_name: Some(s!("CallPossibilities")),
            enclosing_function_hash: Some(s!(
                "ac5f54dc66b0f87fbebe2203db38eb9e3df24c5ced02f55fd5e86351eb05d7fe"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("System.out.println(\"Parent method executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Parent method executed.\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("parentMethod")),
            enclosing_class_name: Some(s!("ParentClass")),
            enclosing_function_hash: Some(s!(
                "a7f85f945aff0016bd880edddf5f477d66022067c405dbdf14450f5ed96e006c"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
        CallStatement {
            function_name: s!("System.out.println(\"Static call executed.\")"),
            arguments: vec![Argument {
                assigned_variable: s!(""),
                value: s!("\"Static call executed.\""),
                datatype: s!("any"),
            }],
            enclosing_function_name: Some(s!("staticAction")),
            enclosing_class_name: Some(s!("StaticTarget")),
            enclosing_function_hash: Some(s!(
                "f747b5e7fd8dea17cae85afcc25fe948e2934bb53e0bca837104bff594937507"
            )),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
        },
    ];

    assert_eq!(calls, expected);
}
