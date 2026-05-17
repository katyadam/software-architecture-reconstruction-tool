use std::collections::HashMap;

use models::{ConfigurationData, RestCall, assignments::VariableAddress};
use sage::resolver::{
    client::SageClient,
    code::CodeSnippet,
    facts::FactBundle,
    query::{QueryKind, SageQuery},
};

use crate::pipeline::{
    pass1::decide_language,
    pass3::llm_enhance::{ranking::rank_and_cap, variables::microservice_for_file},
};

pub(super) fn build_query_for_restcall(
    rc: &RestCall,
    variables: &HashMap<VariableAddress, String>,
    config: &ConfigurationData,
    sage: &SageClient,
) -> Option<SageQuery> {
    let snippet = build_snippet(rc)?;
    let lookup_key = rc
        .target_uri
        .split('/')
        .next()
        .unwrap_or(&rc.target_uri)
        .to_string();
    let microservice = microservice_for_file(&rc.file_path, config);
    let pruned = rank_and_cap(
        variables,
        &snippet.code,
        snippet.language,
        &microservice,
        &rc.file_path,
        sage.variables_budget(),
    );
    let bundle = FactBundle {
        sites: vec![snippet],
    };
    Some(SageQuery {
        bundle,
        kind: QueryKind::ResolveLookup { lookup_key },
        variables_map: pruned,
    })
}

pub(super) fn rewrite_target_uri_with_resolution(original_uri: &str, resolved: &str) -> String {
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
