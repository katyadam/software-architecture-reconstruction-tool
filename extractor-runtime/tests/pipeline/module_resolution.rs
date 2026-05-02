use std::collections::HashMap;

use extractor_runtime::pipeline::{
    build_project_ir, evaluate,
    pass3::{pass_attr, pass_module},
};
use java_extractor::extraction::extract_syntactic as java_extract;
use python_extractor::extraction::parse::extract_syntactic as python_extract;

// ── helpers ───────────────────────────────────────────────────────────────────

fn run_pipeline(
    records: Vec<models::ir::syntax::FileRecord>,
    external_constants: &HashMap<String, String>,
) -> models::ir::evaluted::EvaluatedIR {
    let project_ir = build_project_ir(records);
    let per_file_attrs = pass_attr::resolve_all(&project_ir, external_constants);
    let per_file_module_consts =
        pass_module::resolve_all(&project_ir, external_constants, &per_file_attrs);
    evaluate(
        project_ir,
        external_constants,
        &per_file_attrs,
        &per_file_module_consts,
    )
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Module-level derived global in the SAME file is visible to the file's own
/// function body. Exercises the synthetic `<module>` callable end-to-end
/// (pass_attr populates `settings.x`, pass_module evaluates the `<module>`
/// body to derive `aaa_url`, restcalls.rs layers it into `file_env`).
///
/// `.rstrip("/")` is identity in the current evaluator (unknown method on a
/// string receiver returns the receiver unchanged — see
/// `statix/src/symbolic.rs::visit_call`), so the URI keeps its trailing slash.
/// This test validates propagation, not string-method semantics.
#[test]
fn single_file_derived_global() {
    let code = r#"
class Settings:
    x: str = "http://single/"

settings = Settings()
aaa_url = settings.x.rstrip("/")

def fetch():
    url = aaa_url
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let evaluated = run_pipeline(vec![record], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://single/",
        "same-file module-level global must be visible to the function body"
    );
}

/// Three-file empaia shape: a class file, a singletons file that binds the
/// instance and derives a module-level url, and a consumer file whose REST
/// call uses the derived url via a standard import.
/// TODO: Show this test
#[test]
fn cross_file_propagation() {
    let settings_code = r#"
class Settings:
    aaa_service_url: str = "http://aaa/"
"#;

    let singletons_code = r#"
from settings import Settings

settings = Settings()
aaa_url = settings.aaa_service_url.rstrip("/")
"#;

    let consumer_code = r#"
from singletons import aaa_url

def fetch():
    url = aaa_url
    requests.get(url)
"#;

    let settings = python_extract(settings_code, "settings.py").expect("settings.py parses");
    let singletons =
        python_extract(singletons_code, "singletons.py").expect("singletons.py parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("consumer.py parses");

    let evaluated = run_pipeline(vec![settings, singletons, consumer], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://aaa/",
        "consumer must see singletons.aaa_url resolved (rstrip is identity today)"
    );
}

/// Chained module-level globals: `b_url = a_url + "/v1"` must evaluate `a_url`
/// first to produce a literal for `b_url`. Consumer imports `b_url` and sees
/// `"/api/v1"`. Verifies iterative evaluation inside the synthetic `<module>`
/// callable.
///
/// Uses lowercase names on purpose: pass2's `collect_constants` classifies
/// UPPER_SNAKE_CASE module globals as project constants and stores the raw
/// source text of the RHS, which would clobber pass_module's computed literal.
#[test]
fn chained_module_globals() {
    let singletons_code = r#"
a_url = "/api"
b_url = a_url + "/v1"
"#;

    let consumer_code = r#"
from singletons import b_url

def fetch():
    url = b_url
    requests.get(url)
"#;

    let singletons =
        python_extract(singletons_code, "singletons.py").expect("singletons.py parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("consumer.py parses");

    let evaluated = run_pipeline(vec![singletons, consumer], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api/v1",
        "chained module-level globals must compose to a single literal"
    );
}

/// A module-level global whose RHS cannot reduce to a literal must not be
/// propagated. The consumer's URI should remain unresolved (i.e. the raw
/// variable reference stays in the target).
#[test]
fn non_literal_rhs_skipped() {
    let singletons_code = r#"
def some_function():
    return make_something_at_runtime()

dynamic_url = some_function()
"#;

    let consumer_code = r#"
from singletons import dynamic_url

def fetch():
    url = dynamic_url
    requests.get(url)
"#;

    let singletons = python_extract(singletons_code, "singletons.py").expect("parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("parses");

    let evaluated = run_pipeline(vec![singletons, consumer], &HashMap::new());

    // The REST call might survive, but its URI must not be a resolved literal —
    // the name `dynamic_url` cannot reduce to anything useful.
    assert!(
        evaluated
            .restcalls
            .iter()
            .all(|rc| rc.target_uri != "some_function()" && !rc.target_uri.contains("http")),
        "non-literal RHS must not produce a concrete URI; got: {:?}",
        evaluated.restcalls
    );
}

/// CLI-supplied `external_constants` for a key that also appears as a
/// module-level literal: project constants beat externals beat module consts
/// under the same `or_insert_with` precedence the existing env chain uses.
///
/// Here, no project constant exists, so the external value wins because
/// `build_constants_env` places it into the base env before `pass_module`
/// evaluation occurs. The consumer's REST call resolves to the external.
#[test]
fn cli_override_wins() {
    let singletons_code = r#"
aaa_url = "http://from-module"
"#;

    let consumer_code = r#"
from singletons import aaa_url

def fetch():
    url = aaa_url
    requests.get(url)
"#;

    let singletons = python_extract(singletons_code, "singletons.py").expect("parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("parses");

    let mut external_constants = HashMap::new();
    external_constants.insert("aaa_url".to_string(), "http://from-cli".to_string());

    let evaluated = run_pipeline(vec![singletons, consumer], &external_constants);

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://from-cli",
        "CLI external_constants must override module-level literal"
    );
}

/// Pass_attr seeds `settings.x` for `pass_module`'s evaluation, which then
/// composes it into a derived module-level literal. Verifies the pipeline
/// chain: attr pass -> module pass -> rest-call evaluation.
#[test]
fn attr_precedence_preserved() {
    let code = r#"
class Settings:
    x: str = "http://from-field/"

settings = Settings()
derived = settings.x.rstrip("/")

def fetch():
    url = derived
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("parses");
    let evaluated = run_pipeline(vec![record], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );

    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://from-field/",
        "derived module-level literal must flow through the attr pass (rstrip is identity today)"
    );
}

/// Importing a class (not a constant) must not produce a spurious env entry.
/// The `ImportKind::Constant` filter in Stage B ensures no propagation for
/// entities imported by the consumer.
#[test]
fn non_constant_import_ignored() {
    let lib_code = r#"
class Service:
    pass
"#;

    let consumer_code = r#"
from lib import Service

def fetch():
    url = "/literal"
    requests.get(url)
"#;

    let lib = python_extract(lib_code, "lib.py").expect("parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("parses");

    let evaluated = run_pipeline(vec![lib, consumer], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/literal",
        "class import must not interfere with literal URI resolution"
    );
}

/// `from m import aaa_url as aaa` binds the local name to `aaa`. The REST
/// call uses `aaa`, which must resolve via the codeword->literal mapping.
#[test]
fn renamed_import_binding() {
    let singletons_code = r#"
aaa_url = "http://aliased"
"#;

    let consumer_code = r#"
from singletons import aaa_url as aaa

def fetch():
    url = aaa
    requests.get(url)
"#;

    let singletons = python_extract(singletons_code, "singletons.py").expect("parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("parses");

    let evaluated = run_pipeline(vec![singletons, consumer], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://aliased",
        "renamed import must bind propagated literal to the local alias"
    );
}

/// Two files each define a module-global with the same name `api_url` but
/// different values. The consumer imports specifically from `service_a`.
/// `ImportGraph::lookup` resolves by the consumer's import, keying into
/// `per_file_module_consts[source_file]`, so the wrong service's value must
/// not leak across. Guards against a regression where propagation keyed only
/// by codeword rather than `(source_file, codeword)`.
#[test]
fn same_name_across_services_disambiguated_by_source_file() {
    let service_a_code = r#"
api_url = "http://service-a"
"#;
    let service_b_code = r#"
api_url = "http://service-b"
"#;
    let consumer_code = r#"
from service_a import api_url

def fetch():
    url = api_url
    requests.get(url)
"#;

    let service_a = python_extract(service_a_code, "service_a.py").expect("parses");
    let service_b = python_extract(service_b_code, "service_b.py").expect("parses");
    let consumer = python_extract(consumer_code, "consumer.py").expect("parses");

    let evaluated = run_pipeline(vec![service_a, service_b, consumer], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://service-a",
        "consumer must resolve to the imported source_file's value, not the other service's"
    );
}

/// Full empaia-like chain: Settings class → singletons module → examination module,
/// where the REST call lives inside an inner `async def _` closure that captures
/// a locally-derived URL from the outer `add_routes_examination` function.
///
/// This is the exact shape of `medical-data-service/api/v3/examination.py`:
/// - `settings.py`: declares `Settings` with a non-empty `es_url` field default.
/// - `singletons.py`: instantiates `Settings`, derives `base_url` at module level.
/// - `examination.py`: imports `base_url`, then `add_routes_examination(app)` creates
///   a local `route_base = base_url + "/v3"` and registers `async def _` handlers
///   that use `route_base` in f-string target URIs.
///
/// Exercises two independent mechanisms in a single test:
/// 1. **Cross-file constant propagation** (three-file chain): `settings.es_url` →
///    `singletons.base_url` → imported into `examination.py`'s file-level env.
/// 2. **Closure capture**: `route_base` is a local of `add_routes_examination` (not
///    module-level), derived from the imported `base_url`.  The inner `_` captures it
///    via `build_captured_scopes`, which evaluates the outer function with `file_env`
///    and records captured literals keyed by the inner function's hash.
///
/// Note: the true empaia pattern imports the `settings` *instance* into `examination.py`
/// and derives `base_url` locally from `settings.es_url`.  That variant does not resolve
/// today because the cross-file import of a Settings instance (not a plain literal) does
/// not propagate the dotted key `settings.es_url` into the importer's `file_env` via
/// `build_file_env` — a known limitation tracked separately.
///
/// A second subtlety: `generate_uris` substitutes from `analysis.final_env`, which only
/// contains variables SET inside the function body.  If the inner closure passes an
/// f-string directly to `requests.get(f"{route_base}/...")`, the template variable is
/// never assigned locally and stays unresolved.  The test therefore follows the natural
/// production pattern: `url = f"{route_base}/examinations"; requests.get(url)`.  That
/// assigns `url` in `_`'s body, so `route_base` is evaluated via the captured scope
/// (in `eval_env`) and the resolved string lands in `final_env["url"]`.
/// TODO: Show this test
#[test]
fn empaia_three_file_chain_with_closure_capture() {
    let settings_code = r#"
class Settings:
    es_url: str = "http://examination-service:8000"
"#;

    let singletons_code = r#"
from settings import Settings

settings = Settings()
base_url = settings.es_url.rstrip("/")
"#;

    let examination_code = r#"
from singletons import base_url

def add_routes_examination(app):
    route_base = base_url + "/v3"

    async def _(payload=None):
        url = f"{route_base}/examinations"
        requests.get(url)
"#;

    let settings = python_extract(settings_code, "settings.py").expect("settings.py parses");
    let singletons =
        python_extract(singletons_code, "singletons.py").expect("singletons.py parses");
    let examination =
        python_extract(examination_code, "examination.py").expect("examination.py parses");

    let evaluated = run_pipeline(vec![settings, singletons, examination], &HashMap::new());

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://examination-service:8000/v3/examinations",
        "closure must capture route_base (derived locally in add_routes_examination) \
         after three-file cross-file constant propagation"
    );
}

/// Java files have no synthetic `<module>` callable. `pass_module::resolve_all`
/// must skip them without error and leave Pass 3's Java evaluation unaffected.
/// Mixes a Java file (with a REST call that resolves via a local helper) and
/// a Python file carrying a module-global; both must resolve independently.
#[test]
fn java_file_skipped_by_pass_module() {
    use models::{HttpMethod, RestCall};

    let java_code = r#"
class JavaClient {
    String getBaseUrl() {
        return "/java-api";
    }
    void fetch() {
        String url = getBaseUrl();
    }
}
"#;
    let python_code = r#"
api_url = "/python-api"

def fetch():
    url = api_url
    requests.get(url)
"#;

    let mut java_record = java_extract(java_code, "JavaClient.java").expect("parses");
    java_record.raw_restcalls.push(RestCall {
        function_name: "void fetch()".into(),
        function_hash: String::new(),
        call_arguments: vec![],
        http_method: HttpMethod::GET,
        target_uri: "url".into(),
        file_path: "JavaClient.java".into(),
    });
    let python_record = python_extract(python_code, "client.py").expect("parses");

    let evaluated = run_pipeline(vec![java_record, python_record], &HashMap::new());

    let java_uri = evaluated
        .restcalls
        .iter()
        .find(|rc| rc.file_path == "JavaClient.java")
        .map(|rc| rc.target_uri.as_str());
    let python_uri = evaluated
        .restcalls
        .iter()
        .find(|rc| rc.file_path == "client.py")
        .map(|rc| rc.target_uri.as_str());

    assert_eq!(
        java_uri,
        Some("/java-api"),
        "Java REST call must still resolve via its own helper method"
    );
    assert_eq!(
        python_uri,
        Some("/python-api"),
        "Python REST call must pick up the module-level literal"
    );
}
