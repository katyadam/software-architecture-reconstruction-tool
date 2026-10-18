use crate::call_graph::go_native::GoAnalysisResult;
use models::{call_graph::CallGraphStatus, go_impact::*};
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct GoSelectionDecision {
    pub discovered_tests: Vec<GoTestCase>,
    pub test_inventory_complete: bool,
    pub graph_suggested_tests: Vec<GoSuggestedTest>,
    pub selected_tests: Vec<GoSelectedTest>,
    pub unselected_tests: Vec<GoTestCase>,
    pub selection_mode: GoSelectionMode,
    pub run_all_tests: bool,
    pub diagnostics: Vec<String>,
}
pub fn select_go_tests(
    changes: &[GoChangedElement],
    inventory: &GoInventoryOutcome,
    graph: Option<&GoAnalysisResult>,
) -> GoSelectionDecision {
    let mut d = GoSelectionDecision {
        discovered_tests: vec![],
        test_inventory_complete: false,
        graph_suggested_tests: vec![],
        selected_tests: vec![],
        unselected_tests: vec![],
        selection_mode: GoSelectionMode::NoChanges,
        run_all_tests: false,
        diagnostics: vec![],
    };
    if changes.is_empty() {
        return d;
    }
    d.selection_mode = GoSelectionMode::ConservativeAll;
    d.run_all_tests = true;
    d.discovered_tests = inventory.tests.iter().fold(vec![], |mut list, t| {
        if !list.contains(t) {
            list.push(t.clone())
        }
        list
    });
    d.discovered_tests.sort_by(|a, b| {
        (
            &a.method.declaring_type,
            &a.method.member_name,
            &a.method.source_path,
            a.method.source_line,
        )
            .cmp(&(
                &b.method.declaring_type,
                &b.method.member_name,
                &b.method.source_path,
                b.method.source_line,
            ))
    });
    d.test_inventory_complete =
        inventory.status == GoStageStatus::Ok && inventory.test_inventory_complete;
    let suggestions = validated_graph_suggestions(changes, inventory, graph);
    let good = suggestions.is_some();
    d.graph_suggested_tests = suggestions.unwrap_or_default();
    if !good {
        d.diagnostics.push("graph evidence absent, failed, or inconsistent with independent inventory; no graph suggestions".into())
    }
    let fallback =
        "Go coverage is partial; run every test in the module, not only graph suggestions"
            .to_string();
    d.diagnostics.push(fallback.clone());
    if !d.test_inventory_complete {
        d.diagnostics.push(
            "test inventory incomplete; per-test selection is not a complete execution list".into(),
        )
    }
    for test in &d.discovered_tests {
        let mut reasons = vec![GoSelectionReason::ConservativeFallback {
            diagnostic: fallback.clone(),
        }];
        if let Some(s) = d.graph_suggested_tests.iter().find(|s| &s.test == test) {
            reasons.extend(
                s.changed_targets
                    .iter()
                    .cloned()
                    .map(|target| GoSelectionReason::ChangedCallable { target }),
            );
        }
        d.selected_tests.push(GoSelectedTest {
            test: test.clone(),
            reasons,
        });
    }
    d
}

/// Positive graph evidence only; absence never grants exclusion authority.
pub(super) fn validated_graph_suggestions(
    changes: &[GoChangedElement],
    inventory: &GoInventoryOutcome,
    graph: Option<&GoAnalysisResult>,
) -> Option<Vec<GoSuggestedTest>> {
    let mut suggestions = vec![];
    let targets: BTreeSet<_> = changes.iter().filter_map(|c| c.target.as_deref()).collect();
    let mut good = false;
    if let Some(g) = graph {
        let returned: BTreeSet<_> = g
            .graph
            .reachability
            .iter()
            .map(|r| format!("{}#{}", r.method.declaring_type, r.method.member_name))
            .collect();
        good = (inventory.status == GoStageStatus::Ok && inventory.test_inventory_complete)
            && matches!(
                g.graph.status,
                CallGraphStatus::Ok | CallGraphStatus::NoEntrypoints
            )
            && g.graph.source_root == inventory.source_root
            && g.scope == inventory.scope
            && g.graph.schema_version == 2
            && g.graph.provider_id == "go-native"
            && g.graph.algorithm == "cha"
            && g.tests.len() == inventory.tests.len()
            && g.tests
                .iter()
                .all(|m| inventory.tests.iter().any(|t| &t.method == m))
            && g.graph.reachability.iter().all(|r| {
                r.entrypoints
                    .iter()
                    .all(|m| inventory.tests.iter().any(|t| &t.method == m))
            })
            && returned.iter().map(String::as_str).collect::<BTreeSet<_>>() == targets;
        if good {
            for test in &inventory.tests {
                let reached: BTreeSet<_> = g
                    .graph
                    .reachability
                    .iter()
                    .filter(|r| r.entrypoints.contains(&test.method))
                    .map(|r| format!("{}#{}", r.method.declaring_type, r.method.member_name))
                    .collect();
                if !reached.is_empty() {
                    suggestions.push(GoSuggestedTest {
                        test: test.clone(),
                        changed_targets: reached.into_iter().collect(),
                    });
                }
            }
        }
    }

    good.then_some(suggestions)
}
