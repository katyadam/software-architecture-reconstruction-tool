use models::go_impact::{GoImpactReport, GoSelectionMode};
use serde_json::json;
#[test]
fn go_report_round_trip_keeps_suggestions_distinct_from_selection() {
    let method = json!({"language":"go","declaring_type":"p","member_name":"TestX","descriptor":"func(*testing.T)","source_path":"/p/x_test.go","source_line":1});
    let test = json!({"method":method,"execution_package":"p"});
    let stage = json!({"status":"skipped","diagnostics":[],"duration_ms":0});
    let value = json!({"schema_version":1,"language":"go","module_root":"/p","baseline_revision":"a","candidate_revision":"b","changed_elements":[],"discovered_tests":vec![test.clone();7],"test_inventory_complete":true,"graph_suggested_tests":vec![json!({"test":test,"changed_targets":["p#F"]});5],"selected_tests":vec![json!({"test":test,"reasons":[{"conservative_fallback":{"diagnostic":"opaque"}}]});7],"unselected_tests":[],"selection_mode":"conservative_all","run_all_tests":true,"discovery":stage,"mapping":stage,"analysis":stage,"graph":null,"scope":null,"diagnostics":[]});
    let report: GoImpactReport = serde_json::from_value(value).unwrap();
    assert_eq!(report.graph_suggested_tests.len(), 5);
    assert_eq!(report.selected_tests.len(), 7);
    assert_eq!(report.selection_mode, GoSelectionMode::ConservativeAll);
    assert!(report.run_all_tests);
    assert_eq!(
        report,
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap()
    );
}
