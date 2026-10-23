use models::python_impact::{PythonSuggestedTest, PythonTestCase};
use std::collections::{HashMap, HashSet, VecDeque};

/// Derives graph-based test suggestions from a pyan3 adjacency map.
///
/// For each `changed_target` (dotted, no module prefix, e.g. `"MyClass.method"` or `"function"`),
/// walks the reverse call graph to collect all transitive callers, then matches them against
/// discovered tests. A test is "suggested" if any of its callers transitively reach a changed
/// callable — meaning the graph says that test may exercise the changed code.
///
/// This is **advisory only**: the caller must still select all tests conservatively.
pub fn graph_suggestions(
    changed_targets: &[&str],
    tests: &[PythonTestCase],
    edges: &HashMap<String, Vec<String>>,
) -> Vec<PythonSuggestedTest> {
    if changed_targets.is_empty() || edges.is_empty() {
        return vec![];
    }
    // Build reverse map: callee → callers.
    let reverse = build_reverse(edges);
    let mut suggestions: Vec<PythonSuggestedTest> = vec![];
    for test in tests {
        // Normalise test identity to dotted form (ClassName.test_foo or test_foo).
        let test_dotted = test.qualified_name.replace("::", ".");
        let mut reached_by: Vec<String> = vec![];
        for &target in changed_targets {
            // Find all pyan3 node IDs whose local suffix matches this target.
            let mut target_nodes: Vec<&str> = edges
                .keys()
                .map(String::as_str)
                .filter(|id| node_matches_local(id, target))
                .collect();
            // If the changed target is a method (e.g. `PaymentService.create_payment`),
            // also seed the BFS from the class node (`PaymentService`). pyan3 records
            // instantiation as `[U] ClassName` rather than `[U] ClassName.method`, so
            // tests that call `svc = ClassName(); svc.method()` appear as callers of the
            // class but not the specific method.
            //
            // The class node may only appear as a *callee* in pyan3 output (never as a
            // caller), so we check both edges keys and the reverse map.
            if let Some(class_part) = target.split_once('.').map(|(c, _)| c) {
                for id in edges.keys().map(String::as_str) {
                    if node_matches_local(id, class_part) && !target_nodes.contains(&id) {
                        target_nodes.push(id);
                    }
                }
                // Also include bare class-name references that appear only as callees.
                for callee_list in edges.values() {
                    for callee in callee_list.iter().map(String::as_str) {
                        if node_matches_local(callee, class_part)
                            && !target_nodes.contains(&callee)
                        {
                            target_nodes.push(callee);
                        }
                    }
                }
            }
            if target_nodes.is_empty() {
                continue;
            }
            // BFS backward from those nodes.
            let reachable = reverse_reachable(&target_nodes, &reverse);
            // A test is reachable if:
            // 1. Its own node is in the reachable set, OR
            // 2. It belongs to a class and any sibling method of that class is reachable
            //    (pyan3 often omits test methods that have no outgoing call edges).
            let test_matched = reachable
                .iter()
                .any(|id| node_matches_local(id, test_dotted.as_str()))
                || test.class_name.as_deref().is_some_and(|cls| {
                    reachable.iter().any(|id| class_matches_local(id, cls))
                });
            if test_matched && !reached_by.contains(&target.to_owned()) {
                reached_by.push(target.to_owned());
            }
        }
        if !reached_by.is_empty() {
            suggestions.push(PythonSuggestedTest {
                test: test.clone(),
                changed_targets: reached_by,
            });
        }
    }
    suggestions
}

/// Returns true if `id` (a pyan3 node ID with module prefix) matches `local` (no module prefix).
///
/// `"tests.test_x.TestClass.method"` matches `"TestClass.method"` (suffix after any `.` boundary).
/// `"mymodule.function"` matches `"function"`.
/// `"bare"` matches `"bare"` (exact match, no module).
fn node_matches_local(id: &str, local: &str) -> bool {
    if id == local {
        return true;
    }
    if let Some(prefix) = id.strip_suffix(local) {
        return prefix.ends_with('.');
    }
    false
}

/// Returns true if `id` is any method of `class_name` (i.e. ends with `.<class_name>.<anything>`).
///
/// This catches sibling methods: if `setUpTestData` is reachable, then `test_ratio` in the same
/// class is also considered reachable, because pyan3 often drops test method nodes (they have no
/// outgoing call edges and appear as unreachable leaves).
fn class_matches_local(id: &str, class_name: &str) -> bool {
    // id must contain `.ClassName.something` or be `ClassName.something`
    let class_dot = format!("{class_name}.");
    if let Some(pos) = id.find(class_dot.as_str()) {
        // ensure the char before class_name is a `.` boundary (or start)
        return pos == 0 || id.as_bytes()[pos - 1] == b'.';
    }
    false
}

fn build_reverse<'a>(edges: &'a HashMap<String, Vec<String>>) -> HashMap<&'a str, Vec<&'a str>> {
    let mut rev: HashMap<&str, Vec<&str>> = HashMap::new();
    for (caller, callees) in edges {
        for callee in callees {
            rev.entry(callee.as_str()).or_default().push(caller.as_str());
        }
    }
    rev
}

fn reverse_reachable<'a>(
    starts: &[&'a str],
    reverse: &HashMap<&'a str, Vec<&'a str>>,
) -> HashSet<&'a str> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for &s in starts {
        if visited.insert(s) {
            queue.push_back(s);
        }
    }
    while let Some(node) = queue.pop_front() {
        if let Some(callers) = reverse.get(node) {
            for &caller in callers {
                if visited.insert(caller) {
                    queue.push_back(caller);
                }
            }
        }
    }
    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::python_impact::PythonTestCase;

    fn tc(qualified_name: &str) -> PythonTestCase {
        let (class_name, function_name) = if let Some((c, f)) = qualified_name.split_once("::") {
            (Some(c.to_owned()), f.to_owned())
        } else {
            (None, qualified_name.to_owned())
        };
        PythonTestCase {
            source_path: "tests/test_x.py".into(),
            module_path: "tests.test_x".into(),
            qualified_name: qualified_name.to_owned(),
            function_name,
            class_name,
        }
    }

    #[test]
    fn direct_reach() {
        // test_compute calls service.compute; changed=compute
        let mut edges = HashMap::new();
        edges.insert(
            "tests.test_x.TestClass.test_compute".into(),
            vec!["service.compute".into()],
        );
        edges.insert("service.compute".into(), vec![]);
        let tests = vec![tc("TestClass::test_compute")];
        let suggestions = graph_suggestions(&["compute"], &tests, &edges);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].changed_targets, vec!["compute"]);
    }

    #[test]
    fn transitive_reach() {
        // a→b→c, test calls a, changed=c
        let mut edges = HashMap::new();
        edges.insert("m.a".into(), vec!["m.b".into()]);
        edges.insert("m.b".into(), vec!["m.c".into()]);
        edges.insert("m.c".into(), vec![]);
        edges.insert("tests.test_m.test_a".into(), vec!["m.a".into()]);
        let tests = vec![tc("test_a")];
        let suggestions = graph_suggestions(&["c"], &tests, &edges);
        assert_eq!(suggestions.len(), 1);
    }

    #[test]
    fn no_reach() {
        let mut edges = HashMap::new();
        edges.insert("m.other".into(), vec![]);
        edges.insert("tests.test_m.test_other".into(), vec!["m.other".into()]);
        let tests = vec![tc("test_other")];
        let suggestions = graph_suggestions(&["unrelated_function"], &tests, &edges);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn empty_targets_gives_no_suggestions() {
        let edges = HashMap::new();
        let tests = vec![tc("test_something")];
        let suggestions = graph_suggestions(&[], &tests, &edges);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn node_matches_local_cases() {
        assert!(node_matches_local("tests.test_x.TestClass.method", "TestClass.method"));
        assert!(node_matches_local("mymodule.function", "function"));
        assert!(node_matches_local("bare", "bare"));
        assert!(!node_matches_local("m.other", "other_thing"));
        // Must not match partial segment names
        assert!(!node_matches_local("m.other", "ther"));
    }

    #[test]
    fn class_sibling_reach() {
        // setUpTestData is reachable from changed callable but test_ratio has no call edges.
        // The test methods should still be considered reachable via class membership.
        let mut edges = HashMap::new();
        edges.insert(
            "MyClass.__init__".into(),
            vec!["svc.Model.__init__".into()],
        );
        edges.insert("svc.Model.__init__".into(), vec![]);
        // setUpTestData calls MyClass.__init__; test_ratio has no outgoing edges (absent from map)
        edges.insert(
            "tests.test_m.MyTests.setUpTestData".into(),
            vec!["MyClass.__init__".into()],
        );
        // test_ratio intentionally omitted — pyan3 drops leaf test methods
        let tests = vec![tc("MyTests::test_ratio"), tc("MyTests::test_status")];
        let suggestions = graph_suggestions(&["MyClass.__init__"], &tests, &edges);
        // Both test methods reachable via class sibling setUpTestData
        assert_eq!(suggestions.len(), 2, "{suggestions:?}");
    }

    #[test]
    fn class_instantiation_bridges_to_method() {
        // Test uses `svc = PaymentService(session); await svc.create_payment(...)`.
        // pyan3 records [U] PaymentService (class ref), not [U] PaymentService.create_payment.
        // The class node "PaymentService" appears only as a *callee*, never as a caller key.
        // Changed target is PaymentService.create_payment — BFS must still find the test.
        let mut edges = HashMap::new();
        edges.insert(
            "tests.test_payment.TestPayment.test_create".into(),
            vec!["services.payment.PaymentService".into()],
        );
        // PaymentService.create_payment exists as a node (caller) with no outgoing edges.
        // PaymentService (bare class) only appears as a callee — not a key.
        edges.insert(
            "services.payment.PaymentService.create_payment".into(),
            vec![],
        );
        let tests = vec![tc("TestPayment::test_create")];
        let suggestions =
            graph_suggestions(&["PaymentService.create_payment"], &tests, &edges);
        assert_eq!(suggestions.len(), 1, "{suggestions:?}");
    }

    #[test]
    fn class_instantiation_does_not_over_select() {
        // Two test classes: one uses PaymentService, one uses UnrelatedService.
        // Changing PaymentService.create_payment must NOT select the unrelated test.
        let mut edges = HashMap::new();
        edges.insert(
            "tests.test_payment.TestPayment.test_create".into(),
            vec!["services.payment.PaymentService".into()],
        );
        edges.insert(
            "tests.test_other.TestOther.test_something".into(),
            vec!["services.other.UnrelatedService".into()],
        );
        // Both class nodes appear only as callees (never as caller keys).
        edges.insert(
            "services.payment.PaymentService.create_payment".into(),
            vec![],
        );
        let tests = vec![
            tc("TestPayment::test_create"),
            tc("TestOther::test_something"),
        ];
        let suggestions =
            graph_suggestions(&["PaymentService.create_payment"], &tests, &edges);
        assert_eq!(suggestions.len(), 1, "{suggestions:?}");
        assert_eq!(
            suggestions[0].test.qualified_name,
            "TestPayment::test_create"
        );
    }

    #[test]
    fn class_matches_local_cases() {
        assert!(class_matches_local("MyTests.setUpTestData", "MyTests"));
        assert!(class_matches_local("tests.test_m.MyTests.setUpTestData", "MyTests"));
        assert!(!class_matches_local("OtherTests.setup", "MyTests"));
        // Partial class name must not match
        assert!(!class_matches_local("MyTestsExtra.setup", "MyTests"));
    }
}
