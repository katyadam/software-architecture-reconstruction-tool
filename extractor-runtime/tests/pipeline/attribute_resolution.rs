use std::collections::HashMap;

use extractor_runtime::pipeline::{build_project_ir, evaluate, pass3::pass_attr};
use python_extractor::extraction::parse::extract_syntactic as python_extract;

// ── tests ─────────────────────────────────────────────────────────────────────

/// Annotated binding in a single file resolves `settings.cds_url` to the field default.
///
/// `client.py` defines `Settings` with `cds_url: str = "http://x"`, binds
/// `settings: Settings = Settings()`, and makes a REST call using `settings.cds_url`.
/// `resolve_all` must emit `"settings.cds_url" -> "http://x"` for `client.py`,
/// which `evaluate` then substitutes into the target URI.
#[test]
fn single_file_annotated_binding() {
    let code = r#"
class Settings:
    cds_url: str = "http://x"

settings: Settings = Settings()

def fetch():
    url = settings.cds_url
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let project_ir = build_project_ir(vec![record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://x",
        "annotated binding should resolve to field default"
    );
}

/// Bare constructor form (`settings = Settings()`) exercises the regex fallback.
///
/// Identical to `single_file_annotated_binding` but without a type annotation
/// on the binding.  Stage 1 must derive `"Settings"` via the regex
/// `^([A-Za-z_][A-Za-z0-9_]*)\s*\(`.
#[test]
fn bare_constructor_form() {
    let code = r#"
class Settings:
    cds_url: str = "http://bare"

settings = Settings()

def fetch():
    url = settings.cds_url
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let project_ir = build_project_ir(vec![record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://bare",
        "bare constructor form should resolve to field default"
    );
}

/// Cross-file resolution via ImportGraph: class defined in one file, binding
/// and REST call in the consumer file that imports the class.
///
/// `config.py` defines `Settings` with `cds_url = "http://cross"`.
/// `client.py` imports `Settings`, binds `settings = Settings()`, and makes
/// a REST call using `settings.cds_url`.
/// Stage 2 must resolve `Settings` via the ImportGraph to `config.py` and
/// emit the field default into `client.py`'s attr bucket.
#[test]
fn cross_file_via_import() {
    let config_code = r#"
class Settings:
    cds_url: str = "http://cross"
"#;

    let client_code = r#"
from config import Settings

settings = Settings()

def fetch():
    url = settings.cds_url
    requests.get(url)
"#;

    let config_record = python_extract(config_code, "config.py").expect("config.py should parse");
    let client_record = python_extract(client_code, "client.py").expect("client.py should parse");

    let project_ir = build_project_ir(vec![config_record, client_record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://cross",
        "cross-file import should resolve field default from config.py"
    );
}

/// A field with an empty string default must not emit a dotted constant.
///
/// `Settings.cds_url` has `initial_value = ""`.  After stripping quotes the
/// value is empty, so Stage 3 skips it.  The REST call using `settings.cds_url`
/// must remain unresolved (target_uri unchanged from raw value).
#[test]
fn empty_default_skipped() {
    let code = r#"
class Settings:
    cds_url: str = ""

settings = Settings()

def fetch():
    url = settings.cds_url
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let project_ir = build_project_ir(vec![record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);

    // No dotted constant should have been emitted for cds_url.
    if let Some(file_attrs) = per_file_attrs.get("client.py") {
        assert!(
            !file_attrs.contains_key("settings.cds_url"),
            "empty default must not produce a dotted constant; got: {:?}",
            file_attrs
        );
    }

    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );
    // The restcall may or may not survive evaluation, but it must not resolve to
    // the empty string as a URI.
    for rc in &evaluated.restcalls {
        assert_ne!(
            rc.target_uri, "",
            "resolved URI must not be an empty string"
        );
    }
}

/// CLI `external_constants` wins over the class-field default.
///
/// The class declares `cds_url: str = "http://default"`.  The caller supplies
/// `external_constants["settings.cds_url"] = "http://override"`.
/// The resolved URI must be `"http://override"`.
#[test]
fn cli_override_wins() {
    let code = r#"
class Settings:
    cds_url: str = "http://default"

settings = Settings()

def fetch():
    url = settings.cds_url
    requests.get(url)
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let project_ir = build_project_ir(vec![record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);

    let mut external_constants = HashMap::new();
    external_constants.insert(
        "settings.cds_url".to_string(),
        "http://override".to_string(),
    );

    let evaluated = evaluate(
        project_ir,
        &external_constants,
        &per_file_attrs,
        &HashMap::new(),
    );

    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "http://override",
        "external_constants must override class-field default"
    );
}

/// A global binding `logger = get_logger()` must not be mis-resolved.
///
/// `get_logger` is a `Callable` import, not an `Entity`.  The `ImportKind::Entity`
/// check in Stage 2 must filter it out, and there is no local entity named
/// `get_logger`, so no dotted constants are emitted.
#[test]
fn non_entity_imports_ignored() {
    let code = r#"
from logging_utils import get_logger

logger = get_logger()

def fetch():
    url = "/api/items"
    requests.get(url)
"#;

    let logging_code = r#"
def get_logger():
    return None
"#;

    let record = python_extract(code, "client.py").expect("client.py should parse");
    let logging_record =
        python_extract(logging_code, "logging_utils.py").expect("logging_utils.py should parse");

    let project_ir = build_project_ir(vec![record, logging_record]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);

    // get_logger is a Callable, so Stage 2 skips it entirely — no bucket for client.py.
    assert!(
        !per_file_attrs.contains_key("client.py"),
        "callable import must not produce a file attr bucket; got: {:?}",
        per_file_attrs.get("client.py")
    );

    // The REST call to the literal URI should still resolve correctly.
    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );
    assert_eq!(
        evaluated.restcalls.len(),
        1,
        "exactly one restcall expected"
    );
    assert_eq!(
        evaluated.restcalls[0].target_uri, "/api/items",
        "literal URI should be preserved"
    );
}

/// Per-file isolation: two files with the same `settings` var name but different
/// `Settings` classes must each resolve to their own field default.
///
/// `service_a.py` binds `settings = SettingsA()` where `SettingsA.cds_url = "http://a"`.
/// `service_b.py` binds `settings = SettingsB()` where `SettingsB.cds_url = "http://b"`.
/// After evaluation the REST call in A resolves to `"http://a"` and in B to `"http://b"`.
#[test]
fn per_file_isolation() {
    let service_a_code = r#"
class SettingsA:
    cds_url: str = "http://a"

settings = SettingsA()

def fetch_a():
    url = settings.cds_url
    requests.get(url)
"#;

    let service_b_code = r#"
class SettingsB:
    cds_url: str = "http://b"

settings = SettingsB()

def fetch_b():
    url = settings.cds_url
    requests.get(url)
"#;

    let record_a =
        python_extract(service_a_code, "service_a.py").expect("service_a.py should parse");
    let record_b =
        python_extract(service_b_code, "service_b.py").expect("service_b.py should parse");

    let project_ir = build_project_ir(vec![record_a, record_b]);
    let per_file_attrs = pass_attr::resolve_all(&project_ir);

    // Verify isolation at the attr map level before evaluation.
    let attr_a = per_file_attrs
        .get("service_a.py")
        .expect("service_a.py must have an attr bucket");
    let attr_b = per_file_attrs
        .get("service_b.py")
        .expect("service_b.py must have an attr bucket");

    assert_eq!(
        attr_a.get("settings.cds_url").map(String::as_str),
        Some("http://a"),
        "service_a.py must map settings.cds_url to http://a"
    );
    assert_eq!(
        attr_b.get("settings.cds_url").map(String::as_str),
        Some("http://b"),
        "service_b.py must map settings.cds_url to http://b"
    );

    let evaluated = evaluate(
        project_ir,
        &HashMap::new(),
        &per_file_attrs,
        &HashMap::new(),
    );
    assert_eq!(
        evaluated.restcalls.len(),
        2,
        "one restcall per service expected"
    );

    let mut uris: Vec<&str> = evaluated
        .restcalls
        .iter()
        .map(|r| r.target_uri.as_str())
        .collect();
    uris.sort_unstable();

    assert_eq!(
        uris,
        vec!["http://a", "http://b"],
        "each file's REST call must resolve to its own class default"
    );
}
