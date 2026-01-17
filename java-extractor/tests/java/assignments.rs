use std::collections::HashMap;

use java_extractor::{extraction::assignments::map::get_assignments_map, s};
use models::{Assignment, AssignmentKey, Scope};

use crate::java::utils::{get_tree, load_file};

#[test]
fn test_assignment() {
    let filename = s!("./examples/AllAssignments.java");
    let code = load_file(&filename).unwrap();
    let tree = get_tree(&code);
    let assignments_map = get_assignments_map(&tree, &code);
    let expected_map: HashMap<AssignmentKey, Assignment> = HashMap::from([
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "pi".into(),
            },
            Assignment {
                variable_name: "pi".into(),
                variable_type: "double".into(),
                value: "3.1415".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "a".into(),
            },
            Assignment {
                variable_name: "a".into(),
                variable_type: "int".into(),
                value: "5".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "this.instanceValue".into(),
            },
            Assignment {
                variable_name: "this.instanceValue".into(),
                variable_type: "any".into(),
                value: "50".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("AllAssignments(String name)".into()),
                variable_name: "name".into(),
            },
            Assignment {
                variable_name: "name".into(),
                variable_type: "String".into(),
                value: "".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "small".into(),
            },
            Assignment {
                variable_name: "small".into(),
                variable_type: "byte".into(),
                value: "(byte) b".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void main(String[] args)".into()),
                variable_name: "demo".into(),
            },
            Assignment {
                variable_name: "demo".into(),
                variable_type: "AssignmentDemo".into(),
                value: "new AssignmentDemo(\"Test\")".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "intPi".into(),
            },
            Assignment {
                variable_name: "intPi".into(),
                variable_type: "int".into(),
                value: "(int) pi".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "obj2".into(),
            },
            Assignment {
                variable_name: "obj2".into(),
                variable_type: "AssignmentDemo".into(),
                value: "obj1".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Class("AllAssignments".into()),
                variable_name: "instanceValue".into(),
            },
            Assignment {
                variable_name: "instanceValue".into(),
                variable_type: "int".into(),
                value: "10".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "y".into(),
            },
            Assignment {
                variable_name: "y".into(),
                variable_type: "int".into(),
                value: "2".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "c".into(),
            },
            Assignment {
                variable_name: "c".into(),
                variable_type: "int".into(),
                value: "d = 15".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "date".into(),
            },
            Assignment {
                variable_name: "date".into(),
                variable_type: "java.util.Date".into(),
                value: "new java.util.Date()".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "d".into(),
            },
            Assignment {
                variable_name: "d".into(),
                variable_type: "int".into(),
                value: "15".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "obj1".into(),
            },
            Assignment {
                variable_name: "obj1".into(),
                variable_type: "AssignmentDemo".into(),
                value: "this".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "AssignmentDemo.staticCount".into(),
            },
            Assignment {
                variable_name: "AssignmentDemo.staticCount".into(),
                variable_type: "any".into(),
                value: "200".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void main(String[] args)".into()),
                variable_name: "args".into(),
            },
            Assignment {
                variable_name: "args".into(),
                variable_type: "String[]".into(),
                value: "".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "numbers[1]".into(),
            },
            Assignment {
                variable_name: "numbers[1]".into(),
                variable_type: "any".into(),
                value: "10".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "b".into(),
            },
            Assignment {
                variable_name: "b".into(),
                variable_type: "int".into(),
                value: "10".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("AllAssignments(String name)".into()),
                variable_name: "this.name".into(),
            },
            Assignment {
                variable_name: "this.name".into(),
                variable_type: "any".into(),
                value: "name".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "numbers[0]".into(),
            },
            Assignment {
                variable_name: "numbers[0]".into(),
                variable_type: "any".into(),
                value: "42".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Class("AllAssignments".into()),
                variable_name: "staticCount".into(),
            },
            Assignment {
                variable_name: "staticCount".into(),
                variable_type: "int".into(),
                value: "0".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Class("AllAssignments".into()),
                variable_name: "numbers".into(),
            },
            Assignment {
                variable_name: "numbers".into(),
                variable_type: "int[]".into(),
                value: "new int[5]".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "big".into(),
            },
            Assignment {
                variable_name: "big".into(),
                variable_type: "long".into(),
                value: "a".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Class("AllAssignments".into()),
                variable_name: "name".into(),
            },
            Assignment {
                variable_name: "name".into(),
                variable_type: "String".into(),
                value: "".into(),
            },
        ),
        (
            AssignmentKey {
                scope: Scope::Function("void demoAssignments()".into()),
                variable_name: "x".into(),
            },
            Assignment {
                variable_name: "x".into(),
                variable_type: "int".into(),
                value: "1".into(),
            },
        ),
    ]);

    assert_eq!(assignments_map, expected_map);
}
