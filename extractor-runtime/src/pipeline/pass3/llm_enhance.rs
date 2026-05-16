use std::collections::HashMap;

use futures_util::stream::{self, StreamExt};
use log::warn;
use models::{
    ConfigurationData, RestCall,
    assignments::{AssignmentKey, Scope, VariableAddress},
    ir::project::ProjectIR,
};
use sage::resolver::{
    client::SageClient,
    code::CodeSnippet,
    facts::FactBundle,
    query::{QueryKind, SageQuery},
    response::{SageError, SageResponse},
};

use crate::pipeline::{
    pass1::decide_language,
    pass3::{
        pass_attr::PerFileAttrMap,
        pass_module::PerFileModuleConsts,
        restcalls::{EvalState, is_restcall_evaluated_enough},
    },
};

const MAX_CONCURRENT_LLM_QUERIES: usize = 4;

struct PendingQuery {
    index: usize,
    original_uri: String,
    query: SageQuery,
}

struct QueryOutcome {
    index: usize,
    original_uri: String,
    result: Result<SageResponse, SageError>,
}

pub async fn evaluate_restcalls_with_llm(
    restcalls: &mut Vec<RestCall>,
    variables: HashMap<VariableAddress, String>,
    sage: &SageClient,
) {
    let pending = collect_pending_queries(restcalls, &variables);
    println!(
        "Number of REST calls to evaluate with LLM: {}",
        pending.len()
    );
    let outcomes = dispatch_queries_concurrently(pending, sage).await;
    apply_query_outcomes(restcalls, outcomes);
}

fn collect_pending_queries(
    restcalls: &[RestCall],
    variables: &HashMap<VariableAddress, String>,
) -> Vec<PendingQuery> {
    restcalls
        .iter()
        .enumerate()
        .filter(|(_, rc)| is_restcall_evaluated_enough(rc) == EvalState::NeedsLLM)
        .filter_map(|(index, rc)| {
            let query = build_query_for_restcall(rc, variables).or_else(|| {
                warn!(
                    "sage: skipping {} — cannot read {}",
                    rc.target_uri, rc.file_path
                );
                None
            })?;
            Some(PendingQuery {
                index,
                original_uri: rc.target_uri.clone(),
                query,
            })
        })
        .collect()
}

async fn dispatch_queries_concurrently(
    pending: Vec<PendingQuery>,
    sage: &SageClient,
) -> Vec<QueryOutcome> {
    stream::iter(pending)
        .map(|p| async move {
            let result = sage.query(p.query).await;
            QueryOutcome {
                index: p.index,
                original_uri: p.original_uri,
                result,
            }
        })
        .buffer_unordered(MAX_CONCURRENT_LLM_QUERIES)
        .collect()
        .await
}

fn apply_query_outcomes(restcalls: &mut [RestCall], outcomes: Vec<QueryOutcome>) {
    for outcome in outcomes {
        match outcome.result {
            Ok(resp) => {
                if let Some(resolved) = resp.resolved {
                    restcalls[outcome.index].target_uri =
                        rewrite_target_uri_with_resolution(&outcome.original_uri, &resolved);
                }
            }
            Err(e) => {
                warn!("sage: query for {} failed: {e}", outcome.original_uri);
            }
        }
    }
}

fn build_query_for_restcall(
    rc: &RestCall,
    variables: &HashMap<VariableAddress, String>,
) -> Option<SageQuery> {
    let snippet = build_snippet(rc)?;
    let lookup_key = rc
        .target_uri
        .split('/')
        .next()
        .unwrap_or(&rc.target_uri)
        .to_string();
    let bundle = FactBundle {
        sites: vec![snippet],
    };
    Some(SageQuery {
        bundle,
        kind: QueryKind::ResolveLookup { lookup_key },
        variables_map: variables.clone(),
    })
}

fn rewrite_target_uri_with_resolution(original_uri: &str, resolved: &str) -> String {
    let suffix: String = original_uri
        .split_once('/')
        .map(|(_, rest)| format!("/{rest}"))
        .unwrap_or_default();
    format!("{resolved}{suffix}")
}

fn build_snippet(rc: &RestCall) -> Option<CodeSnippet> {
    let bytes = std::fs::read(&rc.file_path).ok()?;
    let start = rc.source_span.start_byte as usize;
    let end = rc.source_span.end_byte as usize;
    let slice = bytes.get(start..end)?;
    let code = String::from_utf8_lossy(slice).into_owned();
    let language = decide_language(&rc.file_path);
    Some(CodeSnippet { code, language })
}

pub fn build_variable_map(
    config: &ConfigurationData,
    project_ir: &ProjectIR,
    per_file_attrs: &PerFileAttrMap,
    per_file_module_attrs: &PerFileModuleConsts,
) -> HashMap<VariableAddress, String> {
    let mut map = HashMap::new();

    for (name, constant) in &project_ir.constants {
        let addr = VariableAddress {
            microservice: microservice_for_file(&constant.source_file, config),
            file: constant.source_file.clone(),
            key: AssignmentKey {
                scope: Scope::Global,
                variable_name: name.clone(),
            },
        };
        map.insert(addr, constant.value.clone());
    }

    for file in &project_ir.files {
        let microservice = microservice_for_file(&file.file_path, config);
        for (key, assignment) in &file.assignments {
            if matches!(key.scope, Scope::Function(_)) {
                continue;
            }
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file.file_path.clone(),
                key: key.clone(),
            };
            map.insert(addr, assignment.value.clone());
        }
    }

    for (file_path, attr_map) in per_file_attrs {
        let microservice = microservice_for_file(file_path, config);
        for (dotted_key, value) in attr_map {
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file_path.clone(),
                key: AssignmentKey {
                    scope: Scope::Global,
                    variable_name: dotted_key.clone(),
                },
            };
            map.insert(addr, value.clone());
        }
    }

    for (file_path, module_consts) in per_file_module_attrs {
        let microservice = microservice_for_file(file_path, config);
        for (var_name, value) in module_consts {
            let addr = VariableAddress {
                microservice: microservice.clone(),
                file: file_path.clone(),
                key: AssignmentKey {
                    scope: Scope::Global,
                    variable_name: var_name.clone(),
                },
            };
            map.insert(addr, value.clone());
        }
    }

    map
}

fn microservice_for_file(file_path: &str, config: &ConfigurationData) -> String {
    config
        .service_descriptions
        .iter()
        .find(|s| file_path.contains(&s.base_dir_path))
        .map(|s| s.name.clone())
        .unwrap_or_default()
}
