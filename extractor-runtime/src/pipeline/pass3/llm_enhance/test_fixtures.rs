//! A small Python project shared by DDFA tests.
//!
//! `/proj/caller/client.py` holds the residual `self._mds_url + case_id` in
//! `MdsClient.fetch_case`. Imports: `settings` -> settings.py,
//! `Stub` -> Stub.java (other language), `gone` -> missing file.
//! settings.py imports `client` (cycle) and `config` -> config.py.

use std::collections::{BTreeMap, HashMap};

use clients::typesafe::state::{Enclosing, Receiver, ResidualCallState};
use models::{
    Argument, Assignment, AssignmentKey, CallStatement, Callable, ConfigurationData, Entity, Field,
    Import, Namespace, ParsedCallStatement, ParsedCallable, RestCall, Scope,
    configuration::ServiceDescription,
    ir::{
        ast::CallableAst,
        language::Language,
        project::{
            ClassHierarchy, ImportGraph, ImportKind, ProjectIR, ResolvedImport, TypedFileRecord,
        },
    },
    source_code::SourceSpan,
};

use super::facts::FactCtx;
use crate::pipeline::pass3::restcalls::OperandBindings;

pub const ORIGIN: &str = "/proj/caller/client.py";

pub struct World {
    pub ir: ProjectIR,
    pub config: ConfigurationData,
    pub env: HashMap<String, String>,
    pub rc: RestCall,
    pub bindings: OperandBindings,
    pub state: ResidualCallState,
}

impl World {
    pub fn ctx(&self) -> FactCtx<'_> {
        let origin = self
            .ir
            .files
            .iter()
            .find(|f| f.file_path == ORIGIN)
            .expect("origin in IR");
        FactCtx {
            project_ir: &self.ir,
            config: &self.config,
            external_constants: &self.env,
            rc: &self.rc,
            bindings: &self.bindings,
            state: &self.state,
            origin,
            service_dir: "proj/caller",
        }
    }

    /// Adds `importer` --`codeword`--> `source` to the import graph.
    pub fn link(&mut self, importer: &str, codeword: &str, source: &str) {
        self.ir.import_graph.resolved_imports.insert(
            (importer.to_string(), codeword.to_string()),
            ResolvedImport {
                source_file: source.to_string(),
                fully_qualified_name: codeword.to_string(),
                kind: ImportKind::Module,
            },
        );
        if let Some(f) = self.ir.files.iter_mut().find(|f| f.file_path == importer) {
            f.imports.push(import(codeword));
        }
    }
}

pub fn file(path: &str, language: Language) -> TypedFileRecord {
    TypedFileRecord {
        file_path: path.to_string(),
        language,
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        callables: vec![],
        call_statements: vec![],
        assignments: HashMap::new(),
        enums: vec![],
        raw_restcalls: vec![],
        raw_message_edges: vec![],
    }
}

pub fn import(codeword: &str) -> Import {
    Import::from_parts(codeword, "", "", "", codeword)
}

pub fn global(f: &mut TypedFileRecord, name: &str, value: &str) {
    f.assignments.insert(
        AssignmentKey {
            scope: Scope::Global,
            variable_name: name.to_string(),
        },
        Assignment {
            variable_name: name.to_string(),
            variable_type: String::new(),
            value: value.to_string(),
        },
    );
}

/// `fields`: `(name, datatype, initial_value)`.
pub fn entity(
    name: &str,
    path: &str,
    superclasses: &[&str],
    fields: &[(&str, Option<&str>, Option<&str>)],
) -> Entity {
    Entity {
        name: name.to_string(),
        superclasses: superclasses.iter().map(|s| s.to_string()).collect(),
        fields: fields
            .iter()
            .map(|(n, t, v)| Field {
                name: n.to_string(),
                datatype: t.map(str::to_string),
                initial_value: v.map(str::to_string),
                datatype_signature: None,
                is_collection: false,
            })
            .collect(),
        signature: format!("{path}::{name}"),
        file_path: path.to_string(),
    }
}

fn callable(name: &str, signature: &str, hash: &str, path: &str, class: &str) -> ParsedCallable {
    ParsedCallable {
        metadata: Callable {
            name: name.to_string(),
            signature: signature.to_string(),
            namespace: Namespace::Class(class.to_string()),
            parameters: vec![],
            return_type: None,
            is_async: false,
            is_constructor: false,
            hash: hash.to_string(),
            file_path: path.to_string(),
        },
        ast: CallableAst {
            statements: vec![],
            nested: vec![],
        },
    }
}

fn call(function_name: &str, arg: &str, enclosing: &str) -> ParsedCallStatement {
    ParsedCallStatement {
        metadata: CallStatement {
            function_name: function_name.to_string(),
            arguments: vec![Argument {
                assigned_variable: String::new(),
                value: arg.to_string(),
                datatype: None,
            }],
            enclosing_function_name: Some(enclosing.to_string()),
            enclosing_class_name: None,
            enclosing_function_hash: Some("h9".to_string()),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: None,
            source_span: SourceSpan::new(0, 0),
            is_decorator: false,
        },
        call_span: SourceSpan::new(0, 0),
        function_span: None,
    }
}

fn service(name: &str, dir: &str, url: Option<&str>) -> ServiceDescription {
    ServiceDescription {
        name: name.to_string(),
        base_dir_path: dir.to_string(),
        urls: url.map(str::to_string).into_iter().collect(),
    }
}

pub fn world() -> World {
    let fetch_case = callable(
        "fetch_case",
        "fetch_case(self, case_id)",
        "h1",
        ORIGIN,
        "MdsClient",
    );

    let mut client = file(ORIGIN, Language::Python);
    client.callables.push(fetch_case.clone());
    client.entities.push(entity(
        "MdsClient",
        ORIGIN,
        &[],
        &[("_mds_url", None, Some("settings.mds_url"))],
    ));

    let mut settings = file("/proj/caller/settings.py", Language::Python);
    settings.entities.push(entity(
        "Settings",
        "/proj/caller/settings.py",
        &["BaseSettings"],
        &[("mds_url", Some("str"), None)],
    ));
    global(&mut settings, "TIMEOUT", "10");

    let mut config = file("/proj/caller/config.py", Language::Python);
    global(&mut config, "MDS_HOST", "\"http://mds:8000\"");

    let mut api = file("/proj/caller/api.py", Language::Python);
    api.call_statements
        .push(call("self.client.fetch_case", "case_id", "get_case"));

    let stub = file("/proj/caller/Stub.java", Language::Java);

    let ir = ProjectIR {
        files: vec![client, settings, config, api, stub],
        import_graph: ImportGraph {
            resolved_imports: HashMap::new(),
        },
        class_hierarchy: ClassHierarchy {
            parents: HashMap::new(),
            children: HashMap::new(),
        },
        constants: HashMap::new(),
        callable_map: HashMap::new(),
        callables_by_file_hash: HashMap::from([(
            (ORIGIN.to_string(), "h1".to_string()),
            fetch_case,
        )]),
    };

    let mut world = World {
        ir,
        config: ConfigurationData {
            service_descriptions: vec![
                service("caller", "proj/caller", None),
                service("mds", "proj/mds", Some("http://mds:8000")),
                service("auth", "proj/auth", Some("http://auth:8000")),
            ],
        },
        env: HashMap::from([
            ("MDS_URL".to_string(), "http://mds:8000".to_string()),
            ("PYTEST_MDS_URL".to_string(), "http://localhost:8000".to_string()),
            ("OTHER".to_string(), "x".to_string()),
        ]),
        rc: RestCall {
            function_name: "fetch_case".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: "self._mds_url + case_id".to_string(),
            file_path: ORIGIN.to_string(),
            source_span: SourceSpan::new(0, 0),
        },
        bindings: BTreeMap::from([
            ("self._mds_url".to_string(), "settings.mds_url".to_string()),
            ("case_id".to_string(), "unresolved: str".to_string()),
        ]),
        state: ResidualCallState {
            call: "self._client.get(self._mds_url + case_id)".to_string(),
            receiver: Some(Receiver {
                expr: "self._client".to_string(),
                datatype: Some("httpx.AsyncClient".to_string()),
            }),
            residual: "self._mds_url + case_id".to_string(),
            operand_bindings: BTreeMap::new(),
            enclosing: Enclosing {
                function: "fetch_case(self, case_id)".to_string(),
                class: Some("MdsClient".to_string()),
            },
            snippet: Some(
                "def fetch_case(self, case_id):\n    return self._client.get(self._mds_url + case_id)"
                    .to_string(),
            ),
            imports: vec!["settings".to_string()],
            file: "proj/caller/client.py".to_string(),
            facts: vec![],
        },
    };
    world.link(ORIGIN, "settings", "/proj/caller/settings.py");
    world.link(ORIGIN, "Stub", "/proj/caller/Stub.java");
    world.link(ORIGIN, "gone", "/proj/caller/gone.py");
    world.link("/proj/caller/settings.py", "client", ORIGIN);
    world.link(
        "/proj/caller/settings.py",
        "config",
        "/proj/caller/config.py",
    );
    world
}
