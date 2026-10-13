use models::call_graph::{CallGraphOutcome, MethodRef, ResolvedCallEdge};
use models::{Callable, JavaTestCase, Namespace};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAX_UNRESOLVED_ENDPOINT_SAMPLES: usize = 10;

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
    let mut unresolved_source_endpoints = vec![];
    for edge in &outcome.edges {
        let caller = resolve_java_test(&edge.caller, root, tests)
            .or_else(|| resolve(&edge.caller, root, callables));
        let callee = resolve_java_test(&edge.callee, root, tests)
            .or_else(|| resolve(&edge.callee, root, callables));
        for (method, unresolved) in [
            (&edge.caller, caller.is_none()),
            (&edge.callee, callee.is_none()),
        ] {
            if unresolved && belongs_to_source(method, root, callables) {
                unresolved_source_endpoints.push(method);
            }
        }
        match (caller, callee) {
            (Some(source_id), Some(target_id)) => edges.push(ResolvedCallEdge {
                source_id,
                target_id,
            }),
            _ => {}
        }
    }
    (
        edges,
        unresolved_source_endpoint_diagnostics(root, callables, unresolved_source_endpoints),
    )
}

/// Identifies references whose declaring class exists in the analyzed source scope.
fn belongs_to_source(method: &MethodRef, root: &Path, callables: &[Callable]) -> bool {
    if is_bytecode_only_java_member(method) {
        return false;
    }
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
            && matches_declaring_source_file(method, callable)
            && matches!(&callable.namespace, Namespace::Class(name) if name == class)
    })
}

/// Excludes JVM-generated members whose source edits are represented by their enclosing class.
fn is_bytecode_only_java_member(method: &MethodRef) -> bool {
    matches!(
        method.member_name.as_str(),
        "<init>" | "<clinit>" | "$values"
    ) || method.member_name.starts_with("lambda$")
        || (method.member_name == "values"
            && method.descriptor == format!("()[{};", method.declaring_type))
}

/// Narrows package-qualified WALA types to their Java source file, including nested types.
fn matches_declaring_source_file(method: &MethodRef, callable: &Callable) -> bool {
    let declaring_type = method.declaring_type.trim_start_matches('L');
    if !declaring_type.contains('/') {
        return true;
    }
    let outer_type = declaring_type.split('$').next().unwrap_or(declaring_type);
    let source_suffix = format!("src/main/java/{outer_type}.java");
    callable.file_path.ends_with(&source_suffix)
}

/// Summarizes every unresolved source endpoint while keeping diagnostics bounded.
fn unresolved_source_endpoint_diagnostics(
    root: &Path,
    callables: &[Callable],
    methods: Vec<&MethodRef>,
) -> Vec<String> {
    if methods.is_empty() {
        return vec![];
    }
    let mut grouped: BTreeMap<(usize, usize, usize), (usize, BTreeSet<String>)> = BTreeMap::new();
    for method in methods {
        let counts = unresolved_endpoint_counts(method, root, callables);
        let endpoint = unresolved_endpoint_identity(method);
        let (endpoint_count, examples) = grouped.entry(counts).or_default();
        *endpoint_count += 1;
        examples.insert(endpoint);
    }

    let total: usize = grouped.values().map(|(count, _)| count).sum();
    let mut diagnostics = vec![format!("WALA unresolved source endpoints={total}")];
    for ((class_candidates, file_candidates, method_candidates), (endpoint_count, examples)) in
        grouped
    {
        diagnostics.push(format!(
            "WALA unresolved source mismatch: class_candidates={class_candidates}; file_candidates={file_candidates}; method_candidates={method_candidates}; endpoints={endpoint_count}"
        ));
        diagnostics.extend(
            examples
                .into_iter()
                .take(MAX_UNRESOLVED_ENDPOINT_SAMPLES)
                .map(|endpoint| format!("WALA unresolved source endpoint: {endpoint}")),
        );
    }
    diagnostics
}

/// Counts candidates remaining at each source-matching stage for one endpoint.
fn unresolved_endpoint_counts(
    method: &MethodRef,
    root: &Path,
    callables: &[Callable],
) -> (usize, usize, usize) {
    let class = method
        .declaring_type
        .trim_start_matches('L')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    let class_candidates: Vec<_> = callables
        .iter()
        .filter(|callable| {
            callable
                .file_path
                .starts_with(root.to_str().unwrap_or_default())
                && matches!(&callable.namespace, Namespace::Class(name) if name == class)
        })
        .collect();
    let file_candidates: Vec<_> = class_candidates
        .iter()
        .filter(|callable| matches_declaring_source_file(method, callable))
        .collect();
    let method_candidates = file_candidates
        .iter()
        .filter(|callable| callable_matches_method(method, callable))
        .count();
    (
        class_candidates.len(),
        file_candidates.len(),
        method_candidates,
    )
}

/// Uses the JVM identity so examples are stable and directly searchable in WALA output.
fn unresolved_endpoint_identity(method: &MethodRef) -> String {
    format!(
        "{}#{}{}",
        method.declaring_type, method.member_name, method.descriptor
    )
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
                && matches_declaring_source_file(method, c)
                && matches!(c.namespace, Namespace::Class(ref n) if n == class)
                && callable_matches_method(method, c)
        })
        .collect();
    (matches.len() == 1).then(|| matches[0].signature.clone())
}

/// Extracts the declared method identifier independent of its return type's whitespace or generics.
fn callable_method_name(callable: &Callable) -> Option<&str> {
    callable
        .name
        .split_once('(')
        .and_then(|(prefix, _)| prefix.split_whitespace().last())
}

/// Matches a source callable's method identifier and, when valid, the JVM descriptor arity.
fn callable_matches_method(method: &MethodRef, callable: &Callable) -> bool {
    callable_method_name(callable).is_some_and(|name| name == method.member_name)
        && jvm_parameter_count(&method.descriptor)
            .is_none_or(|count| callable.parameters.len() == count)
        && jvm_parameter_types(&method.descriptor).is_none_or(|types| {
            callable
                .parameters
                .iter()
                .map(|parameter| parameter.datatype.as_deref().map(source_type_name))
                .collect::<Option<Vec<_>>>()
                .is_none_or(|source_types| {
                    source_types
                        .iter()
                        .zip(types)
                        .all(|(source_type, jvm_type)| {
                            source_type == &jvm_type || is_erased_type_variable(source_type)
                        })
                })
        })
}

/// A single uppercase identifier is a conventional Java generic type variable (for example R or T).
/// Its bytecode descriptor is the variable's bound, which cannot be recovered from callable metadata.
fn is_erased_type_variable(type_name: &str) -> bool {
    type_name.len() == 1 && type_name.as_bytes()[0].is_ascii_uppercase()
}

/// Counts JVM method parameters while treating array and object descriptors as one parameter.
fn jvm_parameter_count(descriptor: &str) -> Option<usize> {
    jvm_parameter_types(descriptor).map(|types| types.len())
}

/// Decodes JVM parameter descriptors into the corresponding source-level simple type names.
fn jvm_parameter_types(descriptor: &str) -> Option<Vec<String>> {
    let bytes = descriptor.as_bytes();
    if bytes.first() != Some(&b'(') {
        return None;
    }
    let mut index = 1;
    let mut types = vec![];
    while let Some(&byte) = bytes.get(index) {
        if byte == b')' {
            return Some(types);
        }
        let mut dimensions = 0;
        while bytes.get(index) == Some(&b'[') {
            dimensions += 1;
            index += 1;
        }
        let type_name = match bytes.get(index).copied()? {
            b'B' => {
                index += 1;
                "byte".to_owned()
            }
            b'C' => {
                index += 1;
                "char".to_owned()
            }
            b'D' => {
                index += 1;
                "double".to_owned()
            }
            b'F' => {
                index += 1;
                "float".to_owned()
            }
            b'I' => {
                index += 1;
                "int".to_owned()
            }
            b'J' => {
                index += 1;
                "long".to_owned()
            }
            b'S' => {
                index += 1;
                "short".to_owned()
            }
            b'Z' => {
                index += 1;
                "boolean".to_owned()
            }
            b'L' => {
                index += 1;
                let start = index;
                while bytes.get(index).is_some_and(|byte| *byte != b';') {
                    index += 1;
                }
                if bytes.get(index) != Some(&b';') {
                    return None;
                }
                let internal_name = std::str::from_utf8(&bytes[start..index]).ok()?;
                index += 1;
                internal_name
                    .rsplit(['/', '$'])
                    .next()
                    .unwrap_or(internal_name)
                    .to_owned()
            }
            _ => return None,
        };
        types.push(format!("{type_name}{}", "[]".repeat(dimensions)));
    }
    None
}

/// Normalizes generic, qualified, and varargs source syntax to a JVM descriptor's simple type form.
fn source_type_name(source_type: &str) -> String {
    let trimmed = source_type.trim();
    let (base, suffix) = trimmed
        .strip_suffix("...")
        .map(|type_name| (type_name, "[]"))
        .unwrap_or((trimmed, ""));
    let base = base.split('<').next().unwrap_or(base).trim();
    format!("{}{}", base.rsplit('.').next().unwrap_or(base), suffix)
}
