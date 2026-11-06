use models::{
    ChangedElement, ChangedElementKind, JavaTestCase, SelectedTest, SelectionReason,
    TestImpactResult,
    call_graph::{CallGraphOutcome, CallGraphStatus, ResolvedCallEdge},
};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Selects tests reached from changed code and widens to an affected module on any uncertainty.
pub fn select_tests(
    changes: Vec<ChangedElement>,
    tests: Vec<JavaTestCase>,
    outcome: CallGraphOutcome,
    resolved_edges: Vec<ResolvedCallEdge>,
) -> TestImpactResult {
    let affected_modules = affected_modules(&changes);
    let mut selected_reasons: HashMap<usize, Vec<SelectionReason>> = HashMap::new();
    let mut diagnostics = outcome.diagnostics.clone();

    if outcome.status != CallGraphStatus::Ok {
        let diagnostic = failure_diagnostic(&outcome);
        for module in &affected_modules {
            select_module_tests(
                &tests,
                module,
                SelectionReason::AnalysisFailure {
                    diagnostic: diagnostic.clone(),
                },
                &mut selected_reasons,
            );
        }
    }

    for diagnostic in uncertainty_diagnostics(&outcome.diagnostics) {
        for module in &affected_modules {
            select_module_tests(
                &tests,
                module,
                SelectionReason::ModuleFallback {
                    diagnostic: diagnostic.clone(),
                },
                &mut selected_reasons,
            );
        }
    }

    let reverse_edges = reverse_edges(&resolved_edges);
    for change in &changes {
        match change.kind {
            ChangedElementKind::Module => select_module_tests(
                &tests,
                &change.module_root,
                SelectionReason::ModuleFallback {
                    diagnostic: format!("module-level change: {}", change.source_path),
                },
                &mut selected_reasons,
            ),
            ChangedElementKind::Callable | ChangedElementKind::Class => {
                if outcome.status != CallGraphStatus::Ok
                    || !uncertainty_diagnostics(&outcome.diagnostics).is_empty()
                {
                    continue;
                }
                select_reaching_tests(
                    change,
                    &tests,
                    &resolved_edges,
                    &reverse_edges,
                    &mut selected_reasons,
                    &mut diagnostics,
                );
            }
        }
    }

    let mut selected_tests = Vec::new();
    let mut unselected_tests = Vec::new();
    for (index, test) in tests.into_iter().enumerate() {
        if let Some(reasons) = selected_reasons.remove(&index) {
            selected_tests.push(SelectedTest { test, reasons });
        } else {
            unselected_tests.push(test);
        }
    }

    let mut result = TestImpactResult::new(
        String::new(),
        String::new(),
        changes,
        selected_tests,
        unselected_tests,
    );
    result.diagnostics.append(&mut diagnostics);
    result
}

/// Collects the Maven modules whose tests are eligible for a changed-element decision.
fn affected_modules(changes: &[ChangedElement]) -> BTreeSet<String> {
    changes
        .iter()
        .map(|change| change.module_root.clone())
        .collect()
}

/// Converts directed caller-to-callee edges into a callee-to-callers lookup for impact traversal.
fn reverse_edges(edges: &[ResolvedCallEdge]) -> HashMap<&str, Vec<&str>> {
    let mut reverse = HashMap::new();
    for edge in edges {
        reverse
            .entry(edge.target_id.as_str())
            .or_insert_with(Vec::new)
            .push(edge.source_id.as_str());
    }
    reverse
}

/// Selects all discovered tests belonging to a module, retaining each independent reason once.
fn select_module_tests(
    tests: &[JavaTestCase],
    module_root: &str,
    reason: SelectionReason,
    selected_reasons: &mut HashMap<usize, Vec<SelectionReason>>,
) {
    for (index, _) in tests
        .iter()
        .enumerate()
        .filter(|(_, test)| test.module_root == module_root)
    {
        add_reason(selected_reasons, index, reason.clone());
    }
}

/// Traverses all reverse paths from a changed callable or class to discovered test callables.
fn select_reaching_tests(
    change: &ChangedElement,
    tests: &[JavaTestCase],
    edges: &[ResolvedCallEdge],
    reverse: &HashMap<&str, Vec<&str>>,
    selected_reasons: &mut HashMap<usize, Vec<SelectionReason>>,
    diagnostics: &mut Vec<String>,
) {
    let targets = changed_targets(change, edges);
    if targets.is_empty() {
        fallback_unresolved_change(change, tests, selected_reasons, diagnostics);
        return;
    }

    let reached = reverse_reachable(&targets, reverse);
    let mut matched_test = false;
    for (index, test) in tests.iter().enumerate() {
        if test.module_root != change.module_root
            || !reached
                .iter()
                .any(|signature| graph_signature_matches_test(signature, test))
        {
            continue;
        }
        matched_test = true;
        let reason = match change.kind {
            ChangedElementKind::Callable => SelectionReason::ChangedCallable {
                signature: change.callable_signature.clone().unwrap_or_default(),
            },
            ChangedElementKind::Class => SelectionReason::ChangedClass {
                class_name: change.callable_signature.clone().unwrap_or_default(),
            },
            ChangedElementKind::Module => unreachable!("module changes do not use graph traversal"),
        };
        add_reason(selected_reasons, index, reason);
    }
    if !matched_test {
        fallback_unresolved_change(change, tests, selected_reasons, diagnostics);
    }
}

/// Finds graph callables that correspond to one changed element at its available granularity.
fn changed_targets<'a>(change: &ChangedElement, edges: &'a [ResolvedCallEdge]) -> Vec<&'a str> {
    match change.kind {
        ChangedElementKind::Callable => change
            .callable_signature
            .as_deref()
            .into_iter()
            .flat_map(|signature| {
                edges
                    .iter()
                    .filter(move |edge| callable_signature_matches(&edge.target_id, signature))
                    .map(|edge| edge.target_id.as_str())
            })
            .collect(),
        ChangedElementKind::Class => change
            .callable_signature
            .as_deref()
            .into_iter()
            .flat_map(|class_name| {
                edges
                    .iter()
                    .filter(move |edge| signature_belongs_to_class(&edge.target_id, class_name))
                    .map(|edge| edge.target_id.as_str())
            })
            .collect(),
        ChangedElementKind::Module => Vec::new(),
    }
}

/// Returns all callers that can transitively reach a changed target, including the targets.
fn reverse_reachable<'a>(
    targets: &[&'a str],
    reverse: &HashMap<&'a str, Vec<&'a str>>,
) -> HashSet<&'a str> {
    let mut reached: HashSet<_> = targets.iter().copied().collect();
    let mut pending: Vec<_> = targets.to_vec();
    while let Some(target) = pending.pop() {
        for caller in reverse.get(target).into_iter().flatten() {
            if reached.insert(caller) {
                pending.push(caller);
            }
        }
    }
    reached
}

/// Recognizes the existing class-qualified callable-signature convention without guessing owners.
fn signature_belongs_to_class(signature: &str, class_name: &str) -> bool {
    let Some(owner) = signature
        .strip_prefix("class:")
        .and_then(|value| value.split_once('/'))
    else {
        return false;
    };
    owner.0 == class_name || owner.0.rsplit('.').next() == Some(class_name)
}

/// Matches exact graph IDs and the simple `Class.method()` identities produced by Git mapping.
fn callable_signature_matches(graph_signature: &str, changed_signature: &str) -> bool {
    if graph_signature == changed_signature {
        return true;
    }
    let Some((class_name, method_with_parentheses)) = changed_signature.split_once('.') else {
        return false;
    };
    let Some(method_name) = method_with_parentheses.strip_suffix("()") else {
        return false;
    };
    signature_belongs_to_class(graph_signature, class_name)
        && graph_signature
            .split_once('/')
            .is_some_and(|(_, declaration)| declaration.contains(&format!(" {method_name}(")))
}

/// Matches resolved callable IDs and JVM test identities emitted by JUnit source discovery.
fn graph_signature_matches_test(graph_signature: &str, test: &JavaTestCase) -> bool {
    if graph_signature == test.callable_signature {
        return true;
    }
    let Some(class_name) = test.class_name.trim_start_matches('L').rsplit('/').next() else {
        return false;
    };
    signature_belongs_to_class(graph_signature, class_name)
        && graph_signature
            .split_once('/')
            .is_some_and(|(_, declaration)| {
                declaration.contains(&format!(" {}(", test.method_name))
            })
}

/// Records a module fallback when no sound path from the changed element to a discovered test exists.
fn fallback_unresolved_change(
    change: &ChangedElement,
    tests: &[JavaTestCase],
    selected_reasons: &mut HashMap<usize, Vec<SelectionReason>>,
    diagnostics: &mut Vec<String>,
) {
    let diagnostic = format!("unresolved changed element: {}", change.source_path);
    diagnostics.push(diagnostic.clone());
    select_module_tests(
        tests,
        &change.module_root,
        SelectionReason::ModuleFallback { diagnostic },
        selected_reasons,
    );
}

/// Adds a selection reason once while preserving deterministic reason ordering.
fn add_reason(
    selected_reasons: &mut HashMap<usize, Vec<SelectionReason>>,
    index: usize,
    reason: SelectionReason,
) {
    let reasons = selected_reasons.entry(index).or_default();
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

/// Ignores the normal timing record and treats every other adapter diagnostic as uncertainty.
fn uncertainty_diagnostics(diagnostics: &[String]) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.starts_with("analysis_duration_ms="))
        .cloned()
        .collect()
}

/// Creates one concise selection reason from an unsuccessful provider outcome.
fn failure_diagnostic(outcome: &CallGraphOutcome) -> String {
    outcome
        .diagnostics
        .first()
        .cloned()
        .unwrap_or_else(|| format!("call graph status: {:?}", outcome.status))
}

#[cfg(test)]
mod tests {
    use super::{callable_signature_matches, graph_signature_matches_test};
    use models::JavaTestCase;

    #[test]
    fn matches_simple_git_callable_to_extractor_signature() {
        assert!(callable_signature_matches(
            "class:Service/void changed()",
            "Service.changed()"
        ));
    }

    #[test]
    fn matches_discovered_junit_identity_to_extractor_signature() {
        assert!(graph_signature_matches_test(
            "class:ServiceTest/void coversChanged()",
            &JavaTestCase {
                module_root: "module".into(),
                class_name: "Lfixture/ServiceTest".into(),
                method_name: "coversChanged".into(),
                descriptor: "()V".into(),
                callable_signature: "ServiceTest.coversChanged()V".into(),
                source_path: "test.java".into(),
            }
        ));
    }
}
