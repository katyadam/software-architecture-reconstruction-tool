//! Gate for residual REST calls: resolved, empty, or needing resolution.

use std::path::Path;

use clients::typesafe::state::{Enclosing, Receiver, ResidualCallState};
use models::{
    CallStatement, ConfigurationData, RestCall, callables::Namespace, ir::project::ProjectIR,
};

use crate::pipeline::pass3::restcalls::{EvalState, OperandBindings, is_restcall_evaluated_enough};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum ResidualTriage {
    Resolved,
    Empty,
    NeedsResolution,
}

pub(super) fn triage(rc: &RestCall) -> ResidualTriage {
    match is_restcall_evaluated_enough(rc) {
        EvalState::ResolvedURL => ResidualTriage::Resolved,
        EvalState::Junk => ResidualTriage::Empty,
        EvalState::NeedsResolution => ResidualTriage::NeedsResolution,
    }
}

fn typesafe_jev_classify(
    rc: &RestCall,
    bindings: &OperandBindings,
    project_ir: &ProjectIR,
    config: &ConfigurationData,
) -> ResidualTriage {
    let _state = residual_call_state(rc, bindings, project_ir, config);
    ResidualTriage::Empty
}

fn residual_call_state(
    rc: &RestCall,
    bindings: &OperandBindings,
    project_ir: &ProjectIR,
    config: &ConfigurationData,
) -> ResidualCallState {
    let callable = project_ir
        .enclosing_callable(&rc.file_path, &rc.function_hash)
        .map(|c| &c.metadata);

    let source = project_ir
        .files
        .iter()
        .find(|f| f.file_path == rc.file_path)
        .and_then(|f| source_call(rc, &f.call_statements));

    ResidualCallState {
        call: render_call(rc, source.map(|c| strip_args(&c.function_name))),
        receiver: source.and_then(receiver),
        residual: rc.target_uri.clone(),
        operand_bindings: bindings.clone(),
        enclosing: Enclosing {
            function: callable.map_or_else(|| rc.function_name.clone(), |c| c.signature.clone()),
            class: callable.and_then(|c| match &c.namespace {
                Namespace::Class(name) => Some(name.clone()),
                Namespace::Module(_) => None,
            }),
        },
        file: relative_file(&rc.file_path, config),
    }
}

/// `CallStatement` `rc` was identified from: same file, enclosing function and arguments.
// ponytail: same function + same args on two receivers -> first wins; add a call-site span to RestCall if it bites.
fn source_call<'a>(rc: &RestCall, calls: &'a [CallStatement]) -> Option<&'a CallStatement> {
    calls.iter().find(|c| {
        c.enclosing_function_hash.as_deref().unwrap_or_default() == rc.function_hash
            && c.arguments == rc.call_arguments
    })
}

/// `self._client.get` -> `self._client`, typed by pass2 `invoked_on`.
fn receiver(call: &CallStatement) -> Option<Receiver> {
    let (expr, _) = strip_args(&call.function_name).rsplit_once('.')?;
    Some(Receiver {
        expr: expr.to_string(),
        datatype: call.invoked_on.clone(),
    })
}

/// Java `function_name` carries args: `rt.exchange(url, x)` -> `rt.exchange`.
/// Python/Go names don't end in `)`, so `requests.Session().get` stays intact.
// ponytail: paren counting ignores string literals; a paren inside a literal arg mis-splits.
fn strip_args(function_name: &str) -> &str {
    if !function_name.ends_with(')') {
        return function_name;
    }
    let mut depth = 0;
    for (i, c) in function_name.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return &function_name[..i];
                }
            }
            _ => {}
        }
    }
    function_name
}

/// `self._client.get(url, timeout=10)`; HTTP method stands in when callee is unknown.
fn render_call(rc: &RestCall, callee: Option<&str>) -> String {
    let args: Vec<String> = rc
        .call_arguments
        .iter()
        .map(|a| {
            if a.assigned_variable.is_empty() {
                a.value.clone()
            } else {
                format!("{}={}", a.assigned_variable, a.value)
            }
        })
        .collect();
    match callee {
        Some(callee) => format!("{callee}({})", args.join(", ")),
        None => format!("{:?}({})", rc.http_method, args.join(", ")),
    }
}

/// From service base dir on; file name only otherwise. Never the absolute path.
fn relative_file(file_path: &str, config: &ConfigurationData) -> String {
    config
        .service_descriptions
        .iter()
        .find_map(|s| {
            file_path
                .find(&s.base_dir_path)
                .map(|i| file_path[i..].to_string())
        })
        .or_else(|| {
            Path::new(file_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{RestCall, source_code::SourceSpan};

    fn restcall(target_uri: &str) -> RestCall {
        RestCall {
            function_name: "do_call".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: target_uri.to_string(),
            file_path: "/proj/caller/client.py".to_string(),
            source_span: SourceSpan::new(0, 0),
        }
    }

    #[test]
    fn triage_http_target_is_resolved() {
        let rc = restcall("http://medical-data-service:8000/x");
        assert_eq!(triage(&rc), ResidualTriage::Resolved);
    }

    #[test]
    fn triage_empty_target_is_empty() {
        assert_eq!(triage(&restcall("")), ResidualTriage::Empty);
    }

    #[test]
    fn triage_residual_needs_resolution() {
        let rc = restcall("self._mds_url + url");
        assert_eq!(triage(&rc), ResidualTriage::NeedsResolution);
    }

    #[test]
    fn state_carries_bindings_and_strips_absolute_path() {
        use models::configuration::ServiceDescription;
        use models::ir::project::{ClassHierarchy, ImportGraph};
        use std::collections::HashMap;

        let config = ConfigurationData {
            service_descriptions: vec![ServiceDescription {
                name: "caller".to_string(),
                base_dir_path: "proj/caller".to_string(),
                urls: vec![],
            }],
        };
        let pir = ProjectIR {
            files: vec![],
            import_graph: ImportGraph {
                resolved_imports: HashMap::new(),
            },
            class_hierarchy: ClassHierarchy {
                parents: HashMap::new(),
                children: HashMap::new(),
            },
            constants: HashMap::new(),
            callable_map: HashMap::new(),
            callables_by_file_hash: HashMap::new(),
        };

        let bindings =
            OperandBindings::from([("self._mds_url".to_string(), "settings.mds_url".to_string())]);
        let state = residual_call_state(&restcall("self._mds_url + url"), &bindings, &pir, &config);

        assert_eq!(state.call, "GET()");
        assert_eq!(state.file, "proj/caller/client.py");
        assert_eq!(state.enclosing.function, "do_call");
        assert_eq!(state.operand_bindings, bindings);
        assert!(state.receiver.is_none());
    }

    fn call_statement(
        function_name: &str,
        hash: &str,
        args: Vec<models::Argument>,
    ) -> CallStatement {
        CallStatement {
            function_name: function_name.to_string(),
            arguments: args,
            enclosing_function_name: None,
            enclosing_class_name: None,
            enclosing_function_hash: Some(hash.to_string()),
            is_self_invoke: false,
            is_super_invoke: false,
            invoked_on: Some("httpx.Client".to_string()),
            source_span: SourceSpan::new(0, 0),
            is_decorator: false,
        }
    }

    #[test]
    fn source_call_matches_hash_and_args() {
        let arg = models::Argument {
            assigned_variable: String::new(),
            value: "url".to_string(),
            datatype: None,
        };
        let calls = vec![
            call_statement("self._db.get", "h1", vec![]),
            call_statement("self._client.get", "other", vec![arg.clone()]),
            call_statement("self._client.get", "h1", vec![arg.clone()]),
        ];
        let rc = RestCall {
            call_arguments: vec![arg],
            ..restcall("url")
        };

        let found = source_call(&rc, &calls).expect("match");
        assert_eq!(found.enclosing_function_hash.as_deref(), Some("h1"));
        assert_eq!(
            render_call(&rc, Some(strip_args(&found.function_name))),
            "self._client.get(url)"
        );
        let r = receiver(found).expect("receiver");
        assert_eq!(r.expr, "self._client");
        assert_eq!(r.datatype.as_deref(), Some("httpx.Client"));
    }

    #[test]
    fn strip_args_only_strips_trailing_call() {
        assert_eq!(strip_args("rt.exchange(url, a(b))"), "rt.exchange");
        assert_eq!(strip_args("b.uri(x).get(y)"), "b.uri(x).get");
        assert_eq!(
            strip_args("requests.Session().get"),
            "requests.Session().get"
        );
    }
}
