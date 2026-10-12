use models::call_graph::{CallGraphOutcome, MethodRef, ResolvedCallEdge};
use models::{Callable, JavaTestCase, Namespace};
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
    resolve_edges_with_java_tests(outcome, root, callables, &[])
}

/// Maps provider edges to production source callables and discovered JUnit entrypoints.
pub fn resolve_edges_with_java_tests(
    outcome: &CallGraphOutcome,
    root: &Path,
    callables: &[Callable],
    tests: &[JavaTestCase],
) -> (Vec<ResolvedCallEdge>, Vec<String>) {
    let mut edges = vec![];
    let mut diagnostics = vec![];
    for edge in &outcome.edges {
        let caller = resolve_java_test(&edge.caller, root, tests)
            .or_else(|| resolve(&edge.caller, root, callables));
        let callee = resolve_java_test(&edge.callee, root, tests)
            .or_else(|| resolve(&edge.callee, root, callables));
        let unresolved_source_endpoint = (caller.is_none()
            && belongs_to_source(&edge.caller, root, callables))
            || (callee.is_none() && belongs_to_source(&edge.callee, root, callables));
        match (caller, callee) {
            (Some(source_id), Some(target_id)) => edges.push(ResolvedCallEdge {
                source_id,
                target_id,
            }),
            _ if diagnostics.is_empty() && unresolved_source_endpoint => diagnostics
                .push("WALA edge could not be mapped uniquely to source callables".into()),
            _ => {}
        }
    }
    (edges, diagnostics)
}

/// Identifies references whose declaring class exists in the analyzed source scope.
fn belongs_to_source(method: &MethodRef, root: &Path, callables: &[Callable]) -> bool {
    let Some(class) = method
        .declaring_type
        .trim_start_matches('L')
        .rsplit('/')
        .next()
    else {
        return false;
    };
    callables.iter().any(|callable| {
        callable
            .file_path
            .starts_with(root.to_str().unwrap_or_default())
            && matches!(&callable.namespace, Namespace::Class(name) if name == class)
    })
}

/// Resolves a WALA test entrypoint without parsing its source file into a callable.
fn resolve_java_test(method: &MethodRef, root: &Path, tests: &[JavaTestCase]) -> Option<String> {
    let matches: Vec<_> = tests
        .iter()
        .filter(|test| {
            Path::new(&test.module_root).starts_with(root)
                && test.class_name == method.declaring_type
                && test.method_name == method.member_name
                && test.descriptor == method.descriptor
        })
        .collect();
    (matches.len() == 1).then(|| java_test_graph_identity(matches[0]))
}

/// Keeps package-qualified JVM test identities distinct in the resolved graph.
fn java_test_graph_identity(test: &JavaTestCase) -> String {
    format!(
        "{}#{}{}",
        test.class_name, test.method_name, test.descriptor
    )
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
