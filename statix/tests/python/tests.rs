use statix::{
    parse_python,
    python::matcher::PythonCallableMatcher,
    symbolic_evaluation,
    util::{get_python_tree, load_file},
};

#[test]
fn parsing_should_not_fail_for_python_assignment_without_rhs() {
    let file_name = "./examples/uni_assignment_while_parsing.py".to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_python_tree(&code);
    let methods_map = parse_python(&tree, &code);

    assert!(!methods_map.is_empty());
}

#[test]
fn should_eval_statements_inside_try_except_block() {
    let file_name = "./examples/enum_in_restcall_uri.py".to_string();
    let code = load_file(&file_name).unwrap();
    let tree = get_python_tree(&code);
    let methods_map = parse_python(&tree, &code);

    let result = symbolic_evaluation(
        &methods_map,
        "Tuple[str, str] fetch_mapping(str,MappingType)",
        Box::new(PythonCallableMatcher::new()),
    )
    .expect("This test should not fail!");

    result
        .final_env
        .get("url")
        .expect("url should be in the final env!");
}
