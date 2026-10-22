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
            let target_nodes: Vec<&str> = edges
                .keys()
                .map(String::as_str)
                .filter(|id| node_matches_local(id, target))
                .collect();
            if target_nodes.is_empty() {
                continue;
            }
            // BFS backward from those nodes.
            let reachable = reverse_reachable(&target_nodes, &reverse);
            // Check whether any reachable node matches this test.
            let test_matched = reachable
                .iter()
                .any(|id| node_matches_local(id, test_dotted.as_str()));
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
}
