use models::ir::project::ImportKind;
use python_extractor::extraction::parse::extract_syntactic;
use statix::import_graph::build_import_graph;

fn extract(code: &str, file_name: &str) -> models::ir::syntax::FileRecord {
    extract_syntactic(code, file_name).expect("extraction should succeed")
}

#[test]
fn python_class_import_resolves_entity() {
    let service_code = r#"
class UserService():
    def __init__(self, url: str):
        self.url = url
    def find_user(self, user_id: str) -> str:
        return user_id
"#;

    let main_code = r#"
from myapp.services import UserService

def run():
    svc = UserService()
"#;

    let service = extract(service_code, "myapp/services.py");
    let main = extract(main_code, "myapp/main.py");

    let graph = build_import_graph(&[main, service]);

    let ri = graph
        .lookup("myapp/main.py", "UserService")
        .expect("UserService import should resolve");
    assert_eq!(ri.source_file, "myapp/services.py");
    assert!(matches!(ri.kind, ImportKind::Entity));
}

#[test]
fn python_function_import_resolves_callable() {
    let utils_code = r#"
def parse_token(raw: str) -> str:
    return raw.strip()
"#;

    let auth_code = r#"
from myapp.utils import parse_token

def authenticate(header: str) -> str:
    return parse_token(header)
"#;

    let utils = extract(utils_code, "myapp/utils.py");
    let auth = extract(auth_code, "myapp/auth.py");

    let graph = build_import_graph(&[auth, utils]);

    let ri = graph
        .lookup("myapp/auth.py", "parse_token")
        .expect("parse_token import should resolve");
    assert_eq!(ri.source_file, "myapp/utils.py");
    assert!(matches!(ri.kind, ImportKind::Callable));
}

#[test]
fn python_wildcard_import_is_not_resolved() {
    let utils_code = r#"
def helper() -> str:
    return "ok"
"#;

    let consumer_code = r#"
from myapp.utils import *

def run():
    helper()
"#;

    let utils = extract(utils_code, "myapp/utils.py");
    let consumer = extract(consumer_code, "myapp/consumer.py");

    let graph = build_import_graph(&[consumer, utils]);

    assert!(
        graph.resolved_imports.is_empty(),
        "wildcard import must not resolve individual symbols"
    );
}

#[test]
fn python_module_import_resolves_to_module() {
    let config_code = r#"
DEBUG = True
DATABASE_URL = "postgres://localhost/mydb"
"#;

    let main_code = r#"
import myapp.config

def run():
    print(myapp.config.DATABASE_URL)
"#;

    let config = extract(config_code, "myapp/config.py");
    let main = extract(main_code, "myapp/main.py");

    let graph = build_import_graph(&[main, config]);

    let ri = graph
        .lookup("myapp/main.py", "myapp.config")
        .expect("myapp.config module import should resolve");
    assert_eq!(ri.source_file, "myapp/config.py");
    assert!(matches!(ri.kind, ImportKind::Module));
}

#[test]
fn python_third_party_import_is_not_resolved() {
    let consumer_code = r#"
from requests import Session

def fetch(url: str):
    s = Session()
    return s.get(url)
"#;

    let consumer = extract(consumer_code, "myapp/client.py");
    let graph = build_import_graph(&[consumer]);

    assert!(
        graph.resolved_imports.is_empty(),
        "third-party import should produce no resolution"
    );
}

#[test]
fn python_src_prefix_suffix_match() {
    let repo_code = r#"
class ProductRepository():
    def __init__(self):
        self.items = []
    def find(self, product_id: str) -> str:
        return product_id
"#;

    let service_code = r#"
from myapp.repo import ProductRepository

class ProductService():
    def __init__(self):
        self.repo = ProductRepository()
"#;

    let repo = extract(repo_code, "src/myapp/repo.py");
    let service = extract(service_code, "src/myapp/service.py");

    let graph = build_import_graph(&[service, repo]);

    let ri = graph
        .lookup("src/myapp/service.py", "ProductRepository")
        .expect("suffix match should resolve src/-prefixed paths");
    assert_eq!(ri.source_file, "src/myapp/repo.py");
}

#[test]
fn python_multiple_imports_from_same_module() {
    let models_code = r#"
class User():
    def __init__(self, name: str):
        self.name = name

class Order():
    def __init__(self, order_id: str):
        self.order_id = order_id
"#;

    let service_code = r#"
from myapp.models import User, Order

def create_order(user: User) -> Order:
    return Order("new-id")
"#;

    let models = extract(models_code, "myapp/models.py");
    let service = extract(service_code, "myapp/service.py");

    let graph = build_import_graph(&[service, models]);

    assert_eq!(
        graph
            .lookup("myapp/service.py", "User")
            .expect("User should resolve")
            .source_file,
        "myapp/models.py"
    );
    assert_eq!(
        graph
            .lookup("myapp/service.py", "Order")
            .expect("Order should resolve")
            .source_file,
        "myapp/models.py"
    );
}

#[test]
fn python_same_codeword_across_microservices_resolves_independently() {
    // Two microservices both have a class named Order but under distinct modules,
    // which is the common real-world case. Both importers get separate graph entries
    // and each resolves to its own source file.
    let svc_a_model = r#"
class Order():
    def __init__(self, order_id: str):
        self.order_id = order_id
"#;

    let svc_b_model = r#"
class Order():
    def __init__(self, amount: float):
        self.amount = amount
"#;

    let svc_a_consumer = r#"
from orders.models import Order

def handle(order: Order):
    pass
"#;

    let svc_b_consumer = r#"
from payments.models import Order

def process(order: Order):
    pass
"#;

    let order_a = extract(svc_a_model, "service-a/orders/models.py");
    let order_b = extract(svc_b_model, "service-b/payments/models.py");
    let consumer_a = extract(svc_a_consumer, "service-a/orders/handler.py");
    let consumer_b = extract(svc_b_consumer, "service-b/payments/processor.py");

    let graph = build_import_graph(&[consumer_a, consumer_b, order_a, order_b]);

    let ri_a = graph
        .lookup("service-a/orders/handler.py", "Order")
        .expect("service-a Order should resolve");
    let ri_b = graph
        .lookup("service-b/payments/processor.py", "Order")
        .expect("service-b Order should resolve");

    assert_eq!(ri_a.source_file, "service-a/orders/models.py");
    assert_eq!(ri_b.source_file, "service-b/payments/models.py");
}
