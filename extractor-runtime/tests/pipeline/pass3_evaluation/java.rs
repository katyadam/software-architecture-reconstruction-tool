use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate};
use java_extractor::extraction::extract_syntactic as java_extract;

use super::java_restcall;

#[test]
fn java_single_file_restcall_evaluation() {
    let code = r#"
class UserClient {
    String getBaseUrl() {
        return "/api/users";
    }
    void fetchUsers() {
        String url = getBaseUrl();
    }
}
"#;

    let mut project_ir = build_project_ir(vec![
        java_extract(code, "UserClient.java").expect("Java extraction should succeed"),
    ]);
    project_ir.files[0].raw_restcalls.push(java_restcall(
        "void fetchUsers()",
        "url",
        "UserClient.java",
    ));
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api/users",
        "URI should resolve via single-file helper method"
    );
}

#[test]
fn java_cross_file_restcall_evaluation() {
    let base_code = r#"
class BaseService {
    String getBaseUrl() {
        return "/api";
    }
}
"#;

    let client_code = r#"
class UserClient {
    void fetchUsers() {
        String url = getBaseUrl();
    }
}
"#;

    let mut project_ir = build_project_ir(vec![
        java_extract(base_code, "BaseService.java").expect("BaseService.java should parse"),
        java_extract(client_code, "UserClient.java").expect("UserClient.java should parse"),
    ]);
    project_ir
        .files
        .iter_mut()
        .find(|file| file.file_path == "UserClient.java")
        .expect("UserClient.java should exist")
        .raw_restcalls
        .push(java_restcall("void fetchUsers()", "url", "UserClient.java"));
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api",
        "URI should resolve via cross-file helper method"
    );
}

#[test]
fn java_callable_collision_local_priority() {
    let global_code = r#"
class GlobalService {
    String getBaseUrl() {
        return "/global";
    }
}
"#;

    let local_code = r#"
class LocalClient {
    String getBaseUrl() {
        return "/local";
    }
    void fetchItems() {
        String url = getBaseUrl();
    }
}
"#;

    let mut project_ir = build_project_ir(vec![
        java_extract(global_code, "GlobalService.java").expect("GlobalService.java should parse"),
        java_extract(local_code, "LocalClient.java").expect("LocalClient.java should parse"),
    ]);
    project_ir
        .files
        .iter_mut()
        .find(|file| file.file_path == "LocalClient.java")
        .expect("LocalClient.java should exist")
        .raw_restcalls
        .push(java_restcall(
            "void fetchItems()",
            "url",
            "LocalClient.java",
        ));
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/local",
        "local callable definition must take priority over global"
    );
}
