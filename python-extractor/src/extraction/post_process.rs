use std::collections::HashMap;

use models::{
    HttpMethod,
    ir::project::TypedFileRecord,
};

/// Applies Python-specific endpoint transformations after project-wide type
/// resolution. The runtime only invokes this generic Python hook; framework
/// details remain inside the Python extractor.
pub(super) fn post_process(files: &mut [&mut TypedFileRecord]) {
    enrich_urlpattern_methods(files);
}

fn enrich_urlpattern_methods(files: &mut [&mut TypedFileRecord]) {
    let api_view_methods = collect_api_view_methods(files);
    if api_view_methods.is_empty() {
        return;
    }

    for file in files.iter_mut() {
        let mut enriched = Vec::new();
        for endpoint in file.endpoints.drain(..) {
            if endpoint.uri.is_empty() && endpoint.router_variable.is_none() {
                continue;
            }
            if endpoint.router_variable.as_deref() == Some("urlpatterns")
                && let Some(methods) = api_view_methods.get(&endpoint.function_name)
            {
                for method in methods {
                    let mut cloned = endpoint.clone();
                    cloned.http_method = method.clone();
                    enriched.push(cloned);
                }
            } else {
                enriched.push(endpoint);
            }
        }
        file.endpoints = enriched;
    }
}

fn collect_api_view_methods(files: &[&mut TypedFileRecord]) -> HashMap<String, Vec<HttpMethod>> {
    let mut methods = HashMap::new();
    for file in files {
        for endpoint in &file.endpoints {
            if endpoint.uri.is_empty() && endpoint.router_variable.is_none() {
                methods
                    .entry(endpoint.function_name.clone())
                    .or_insert_with(Vec::new)
                    .push(endpoint.http_method.clone());
            }
        }
    }
    methods
}
