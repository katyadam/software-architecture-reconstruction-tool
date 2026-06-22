//! Signal extraction for unresolved (residual) REST calls.
//!
//! Given a residual [`models::RestCall`] -- one whose `target_uri` could not be
//! resolved to a concrete host by symbolic evaluation -- this gathers the
//! evidence around the call site that hints at which microservice it targets.
//! It is the foundation for a later deterministic matcher and an LLM classifier;
//! this module only produces the evidence, it does not decide anything.

use std::collections::BTreeSet;

use models::{
    ConfigurationData, RestCall,
    callables::Namespace,
    ir::project::ProjectIR,
};

use crate::pipeline::pass1::decide_language;
use crate::pipeline::pass3::llm_enhance::variables::microservice_for_file;

/// Evidence gathered around a single residual REST call site.
pub(super) struct CallSiteSignals {
    /// The CALLER service. The answer is never the origin, so this is excluded.
    pub origin_service: String,
    /// Enclosing class name, if the call sits inside a class -- strongest signal.
    pub client_class: Option<String>,
    /// Import module/name strings present in the call's source file.
    pub imports: Vec<String>,
    /// Identifiers appearing in the residual `target_uri` expression.
    pub operand_identifiers: Vec<String>,
    /// Config service names: the closed set the answer must come from.
    pub candidate_services: Vec<String>,
}

/// Extract [`CallSiteSignals`] for a residual REST call.
pub(super) fn extract(
    rc: &RestCall,
    project_ir: &ProjectIR,
    config: &ConfigurationData,
) -> CallSiteSignals {
    let origin_service = microservice_for_file(&rc.file_path, config);

    // NOTE: `project_ir.callable_map` is keyed by *mangled name*, not by hash
    // (see pass2::callables::build_project_global_callables), so a direct
    // `callable_map.get(function_hash)` never hits. The per-file `callables`
    // carry the real `hash` and `namespace`, so resolve the enclosing callable
    // by scanning the owning file's callables for a matching hash.
    let client_class = (!rc.function_hash.is_empty())
        .then(|| {
            project_ir
                .files
                .iter()
                .find(|f| f.file_path == rc.file_path)
                .and_then(|f| {
                    f.callables
                        .iter()
                        .find(|c| c.metadata.hash == rc.function_hash)
                })
        })
        .flatten()
        .and_then(|callable| match &callable.metadata.namespace {
            Namespace::Class(name) => Some(name.clone()),
            Namespace::Module(_) => None,
        });

    let imports = project_ir
        .files
        .iter()
        .find(|f| f.file_path == rc.file_path)
        .map(|f| f.imports.iter().map(render_import).collect())
        .unwrap_or_default();

    let language = decide_language(&rc.file_path);
    let operand_identifiers: Vec<String> =
        statix::identifiers::identifiers_in_snippet(&rc.target_uri, language)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

    let candidate_services = config
        .service_descriptions
        .iter()
        .map(|s| s.name.clone())
        .collect();

    CallSiteSignals {
        origin_service,
        client_class,
        imports,
        operand_identifiers,
        candidate_services,
    }
}

/// Render an [`models::Import`] into a readable `module.name` string, appending
/// the local codeword when it differs (i.e. the symbol is aliased).
fn render_import(import: &models::Import) -> String {
    let base = if import.orig_module.is_empty() {
        import.orig_name.clone()
    } else if import.orig_name.is_empty() {
        import.orig_module.clone()
    } else {
        format!("{}.{}", import.orig_module, import.orig_name)
    };

    let aliased = !import.codeword.is_empty()
        && import.codeword != import.orig_name
        && import.codeword != import.orig_module;
    if aliased {
        format!("{base} as {}", import.codeword)
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use models::{
        Callable, Import, ParsedCallable,
        callables::Namespace,
        configuration::ServiceDescription,
        ir::{
            ast::CallableAst,
            language::Language,
            project::{ClassHierarchy, ImportGraph, ProjectIR, TypedFileRecord},
        },
        source_code::SourceSpan,
    };

    fn empty_ast() -> CallableAst {
        CallableAst {
            statements: vec![],
            nested: vec![],
        }
    }

    fn callable(hash: &str, file_path: &str, namespace: Namespace) -> ParsedCallable {
        ParsedCallable {
            metadata: Callable {
                name: "do_call".to_string(),
                signature: "do_call()".to_string(),
                namespace,
                parameters: vec![],
                return_type: None,
                is_async: false,
                is_constructor: false,
                hash: hash.to_string(),
                file_path: file_path.to_string(),
            },
            ast: empty_ast(),
        }
    }

    fn typed_file(
        file_path: &str,
        imports: Vec<Import>,
        callables: Vec<ParsedCallable>,
    ) -> TypedFileRecord {
        TypedFileRecord {
            file_path: file_path.to_string(),
            language: Language::Python,
            imports,
            entities: vec![],
            endpoints: vec![],
            callables,
            call_statements: vec![],
            assignments: HashMap::new(),
            enums: vec![],
            raw_restcalls: vec![],
        }
    }

    fn project_ir(files: Vec<TypedFileRecord>) -> ProjectIR {
        // callable_map is keyed by mangled name in production; the signal
        // extractor deliberately does not rely on it, so it stays empty here.
        ProjectIR {
            files,
            import_graph: ImportGraph {
                resolved_imports: HashMap::new(),
            },
            class_hierarchy: ClassHierarchy {
                parents: HashMap::new(),
                children: HashMap::new(),
            },
            constants: HashMap::new(),
            callable_map: HashMap::new(),
        }
    }

    fn config() -> ConfigurationData {
        ConfigurationData {
            service_descriptions: vec![
                ServiceDescription {
                    name: "caller-service".to_string(),
                    base_dir_path: "/proj/caller".to_string(),
                    urls: vec![],
                },
                ServiceDescription {
                    name: "medical-data-service".to_string(),
                    base_dir_path: "/proj/mds".to_string(),
                    urls: vec![],
                },
            ],
        }
    }

    fn restcall(file_path: &str, function_hash: &str, target_uri: &str) -> RestCall {
        RestCall {
            function_name: "do_call".to_string(),
            function_hash: function_hash.to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: target_uri.to_string(),
            file_path: file_path.to_string(),
            source_span: SourceSpan::new(0, 0),
        }
    }

    #[test]
    fn class_recovered_from_callable_map() {
        let file = "/proj/caller/client.py";
        let pir = project_ir(vec![typed_file(
            file,
            vec![],
            vec![callable("h1", file, Namespace::Class("ApiClient".to_string()))],
        )]);
        let rc = restcall(file, "h1", "self.url + '/x'");
        let signals = extract(&rc, &pir, &config());
        assert_eq!(signals.client_class.as_deref(), Some("ApiClient"));
        assert_eq!(signals.origin_service, "caller-service");
    }

    #[test]
    fn module_namespace_yields_no_class() {
        let file = "/proj/caller/client.py";
        let pir = project_ir(vec![typed_file(
            file,
            vec![],
            vec![callable("h1", file, Namespace::Module("client".to_string()))],
        )]);
        let rc = restcall(file, "h1", "url + '/x'");
        let signals = extract(&rc, &pir, &config());
        assert_eq!(signals.client_class, None);
    }

    #[test]
    fn missing_hash_yields_no_class() {
        let file = "/proj/caller/client.py";
        let pir = project_ir(vec![typed_file(file, vec![], vec![])]);
        let rc = restcall(file, "absent-hash", "url + '/x'");
        let signals = extract(&rc, &pir, &config());
        assert_eq!(signals.client_class, None);
    }

    #[test]
    fn operand_identifiers_extracted() {
        let file = "/proj/caller/client.py";
        let pir = project_ir(vec![typed_file(file, vec![], vec![])]);
        let rc = restcall(file, "absent", "self._mds_url + url");
        let signals = extract(&rc, &pir, &config());
        // Sorted + de-duplicated. `self`, `_mds_url`, `url` are identifiers.
        assert_eq!(
            signals.operand_identifiers,
            vec![
                "_mds_url".to_string(),
                "self".to_string(),
                "url".to_string(),
            ]
        );
    }

    #[test]
    fn imports_rendered() {
        let file = "/proj/caller/client.py";
        let imports = vec![
            Import::from_parts("singletons", "base_url", "", "", "base_url"),
            Import::from_parts("os", "environ", "", "env", "env"),
        ];
        let pir = project_ir(vec![typed_file(file, imports, vec![])]);
        let rc = restcall(file, "absent", "base_url");
        let signals = extract(&rc, &pir, &config());
        assert!(signals.imports.contains(&"singletons.base_url".to_string()));
        assert!(signals.imports.contains(&"os.environ as env".to_string()));
    }

    #[test]
    fn candidate_services_mirror_config() {
        let file = "/proj/caller/client.py";
        let pir = project_ir(vec![typed_file(file, vec![], vec![])]);
        let rc = restcall(file, "absent", "url");
        let signals = extract(&rc, &pir, &config());
        assert_eq!(
            signals.candidate_services,
            vec![
                "caller-service".to_string(),
                "medical-data-service".to_string(),
            ]
        );
    }
}
