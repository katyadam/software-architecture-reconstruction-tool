use java_extractor::extraction::extract_syntactic;
use models::ir::project::ImportKind;
use statix::import_graph::build_import_graph;

fn extract(code: &str, file_name: &str) -> models::ir::syntax::FileRecord {
    extract_syntactic(code, file_name).expect("extraction should succeed")
}

#[test]
fn java_class_import_resolves_entity() {
    let service_code = r#"
package com.example.service;

public class UserService {
    private String prefix;
    public String findUser(String id) {
        return id;
    }
}
"#;

    let main_code = r#"
package com.example;

import com.example.service.UserService;

public class Main {
    public void run() {
        UserService svc = new UserService();
    }
}
"#;

    let service = extract(service_code, "com/example/service/UserService.java");
    let main = extract(main_code, "com/example/Main.java");

    let graph = build_import_graph(&[main, service]);

    let ri = graph
        .resolved_imports
        .get("UserService")
        .expect("UserService import should resolve");
    assert_eq!(ri.source_file, "com/example/service/UserService.java");
    assert!(matches!(ri.kind, ImportKind::Entity));
}

#[test]
fn java_static_method_import_resolves_callable() {
    let utils_code = r#"
package com.example.util;

public class StringUtils {
    public static String truncate(String s, int len) {
        return s.substring(0, len);
    }
}
"#;

    let consumer_code = r#"
package com.example;

import com.example.util.StringUtils;

public class Consumer {
    public String shorten(String input) {
        return StringUtils.truncate(input, 10);
    }
}
"#;

    let utils = extract(utils_code, "com/example/util/StringUtils.java");
    let consumer = extract(consumer_code, "com/example/Consumer.java");

    let graph = build_import_graph(&[consumer, utils]);

    // StringUtils is imported as a class (entity), not a bare callable import
    assert!(
        graph.resolved_imports.contains_key("StringUtils"),
        "StringUtils should be resolved"
    );
}

#[test]
fn java_wildcard_import_is_not_resolved() {
    let service_code = r#"
package com.example.service;

public class OrderService {}
"#;

    let main_code = r#"
package com.example;

import com.example.service.*;

public class Main {}
"#;

    let service = extract(service_code, "com/example/service/OrderService.java");
    let main = extract(main_code, "com/example/Main.java");

    let graph = build_import_graph(&[main, service]);

    assert!(
        !graph.resolved_imports.contains_key("OrderService"),
        "wildcard import must not resolve individual symbols"
    );
}

#[test]
fn java_maven_layout_suffix_match() {
    // Simulates Maven project where file paths have src/main/java/ prefix
    // but import statements use the plain package path.
    let repo_code = r#"
package com.example.repo;

public class ProductRepository {
    public String find(String id) {
        return id;
    }
}
"#;

    let service_code = r#"
package com.example.service;

import com.example.repo.ProductRepository;

public class ProductService {
    private ProductRepository repo;
}
"#;

    let repo = extract(repo_code, "src/main/java/com/example/repo/ProductRepository.java");
    let service = extract(service_code, "src/main/java/com/example/service/ProductService.java");

    let graph = build_import_graph(&[service, repo]);

    assert!(
        graph.resolved_imports.contains_key("ProductRepository"),
        "suffix match should resolve Maven-layout paths"
    );
    assert_eq!(
        graph.resolved_imports["ProductRepository"].source_file,
        "src/main/java/com/example/repo/ProductRepository.java"
    );
}

#[test]
fn java_multiple_imports_all_resolve() {
    let entity_code = r#"
package com.example.model;

public class Order {
    private String id;
}
"#;

    let service_code = r#"
package com.example.service;

public class PaymentService {
    public boolean charge(String orderId) {
        return true;
    }
}
"#;

    let consumer_code = r#"
package com.example;

import com.example.model.Order;
import com.example.service.PaymentService;

public class Checkout {
    private PaymentService payments;

    public void process(Order order) {}
}
"#;

    let entity = extract(entity_code, "com/example/model/Order.java");
    let service = extract(service_code, "com/example/service/PaymentService.java");
    let consumer = extract(consumer_code, "com/example/Checkout.java");

    let graph = build_import_graph(&[consumer, entity, service]);

    assert!(graph.resolved_imports.contains_key("Order"), "Order should resolve");
    assert!(graph.resolved_imports.contains_key("PaymentService"), "PaymentService should resolve");
    assert_eq!(graph.resolved_imports["Order"].source_file, "com/example/model/Order.java");
    assert_eq!(graph.resolved_imports["PaymentService"].source_file, "com/example/service/PaymentService.java");
}
