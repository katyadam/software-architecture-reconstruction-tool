use models::call_graph::{CallGraphOutcome, MethodRef, ResolvedCallEdge};
use models::{Callable, Namespace};
use std::path::Path;

/// Maps provider method references to unique VoyantClair callable signatures.
///
/// Unresolvable endpoints are deliberately omitted so the synthesized graph remains
/// conservative and diagnostics explain the omitted edges.
pub fn resolve_edges(
    outcome: &CallGraphOutcome,
    root: &Path,
    callables: &[Callable],
) -> (Vec<ResolvedCallEdge>, Vec<String>) {
    let mut edges = vec![];
    let mut diagnostics = vec![];
    for edge in &outcome.edges {
        match (
            resolve(&edge.caller, root, callables),
            resolve(&edge.callee, root, callables),
        ) {
            (Some(source_id), Some(target_id)) => edges.push(ResolvedCallEdge {
                source_id,
                target_id,
            }),
            _ => diagnostics
                .push("WALA edge could not be mapped uniquely to source callables".into()),
        }
    }
    (edges, diagnostics)
}

/// Resolves one provider method reference only when exactly one source callable matches it.
fn resolve(method: &MethodRef, root: &Path, callables: &[Callable]) -> Option<String> {
    let class = method
        .declaring_type
        .trim_start_matches('L')
        .rsplit('/')
        .next()?;
    // Ambiguous matches are rejected rather than guessed to avoid false call-graph edges.
    let matches: Vec<_> = callables
        .iter()
        .filter(|c| {
            c.file_path.starts_with(root.to_str().unwrap_or_default())
                && matches!(c.namespace, Namespace::Class(ref n) if n == class)
                && c.name
                    .split_whitespace()
                    .nth(1)
                    .is_some_and(|n| n.starts_with(&(method.member_name.clone() + "(")))
        })
        .collect();
    (matches.len() == 1).then(|| matches[0].signature.clone())
}
