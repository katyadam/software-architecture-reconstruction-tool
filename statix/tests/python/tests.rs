use statix::{
    parse_python,
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
