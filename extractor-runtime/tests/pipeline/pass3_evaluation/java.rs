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

    let mut record = java_extract(code, "UserClient.java").expect("Java extraction should succeed");
    record
        .raw_restcalls
        .push(java_restcall("void fetchUsers()", "url", "UserClient.java"));

    let project_ir = build_project_ir(vec![record]);
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

    let base_record =
        java_extract(base_code, "BaseService.java").expect("BaseService.java should parse");
    let mut client_record =
        java_extract(client_code, "UserClient.java").expect("UserClient.java should parse");
    client_record
        .raw_restcalls
        .push(java_restcall("void fetchUsers()", "url", "UserClient.java"));

    let project_ir = build_project_ir(vec![base_record, client_record]);
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

    let global_record =
        java_extract(global_code, "GlobalService.java").expect("GlobalService.java should parse");
    let mut local_record =
        java_extract(local_code, "LocalClient.java").expect("LocalClient.java should parse");
    local_record.raw_restcalls.push(java_restcall(
        "void fetchItems()",
        "url",
        "LocalClient.java",
    ));

    let project_ir = build_project_ir(vec![global_record, local_record]);
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
