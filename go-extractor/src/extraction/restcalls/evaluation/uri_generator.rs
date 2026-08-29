use std::collections::HashMap;

use statix::symbolic::AnalysisResult;

pub fn generate_target_uris(
    template: &str,
    _analysis: &AnalysisResult,
    _enums: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut uris = vec![template.to_string()];
    if let Some((base, _query)) = template.split_once('?') {
        uris.push(base.to_string());
    }
    uris.sort();
    uris.dedup();
    uris
}
