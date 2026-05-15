use std::collections::HashMap;

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
};

use crate::pipeline::{
    pass1::decide_language,
    pass3::{
        pass_attr::PerFileAttrMap,
        pass_module::PerFileModuleConsts,
        restcalls::{EvalState, is_restcall_evaluated_enough},
    },
};

pub async fn evaluate_restcalls_with_llm(
    restcalls: &mut Vec<RestCall>,
    variables: HashMap<models::assignments::VariableAddress, String>,
    sage: &SageClient,
) {
    let n = restcalls
        .iter()
        .filter(|rc| is_restcall_evaluated_enough(*rc) == EvalState::NeedsLLM)
        .count();
    println!("Number of REST calls to evaluate with LLM: {n}");
    let mut cur = 0;
    for rc in restcalls.iter_mut() {
        if is_restcall_evaluated_enough(rc) != EvalState::NeedsLLM {
            continue;
        }
        println!("Evaluating REST call: {rc:#?} -- {cur}/{n}");
        cur += 1;
        let snippet = match build_snippet(rc) {
            Some(s) => s,
            None => {
                warn!(
                    "sage: skipping {} — cannot read {}",
                    rc.target_uri, rc.file_path
                );
                continue;
            }
        };

        let lookup_key = rc
            .target_uri
            .split('/')
            .next()
            .unwrap_or(&rc.target_uri)
            .to_string();

        println!("Looking to resolve: {lookup_key}");

        let bundle = FactBundle {
            sites: vec![snippet],
            frameworks: vec![],
            scraped_variables: HashMap::new(),
            others: vec![],
        };

        let query = SageQuery {
            bundle,
            kind: QueryKind::ResolveLookup { lookup_key },
            variables_map: variables.clone(),
        };

        match sage.query(query).await {
            Ok(resp) => {
                if let Some(resolved) = resp.resolved {
                    println!("Lookup key resolved to: {resolved}");
                    let suffix: String = rc
                        .target_uri
                        .splitn(2, '/')
                        .nth(1)
                        .map(|s| format!("/{s}"))
                        .unwrap_or_default();
                    rc.target_uri = format!("{resolved}{suffix}");
                    println!("Full target URL is: {}", rc.target_uri);
                }
            }
            Err(e) => {
                println!("sage: query for {} failed: {e}", rc.target_uri);
                warn!("sage: query for {} failed: {e}", rc.target_uri);
            }
        }
    }
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
