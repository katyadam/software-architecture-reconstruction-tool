use models::go_package_impact::*;

#[test]
fn package_capability_requires_explicit_complete_evidence() {
    let v = serde_json::json!({"schema_version":1,"provider_id":"go-native","status":"ok","source_root":"/p","scope":null,"effective_build_context":null,"complete":true,"packages":[],"execution_packages":[],"blockers":[],"diagnostics":[]});
    let evidence: GoPackageInventoryOutcome = serde_json::from_value(v.clone()).unwrap();
    assert!(evidence.complete);
    assert_eq!(serde_json::to_value(&evidence).unwrap(), v);
    let mut missing = v;
    missing.as_object_mut().unwrap().remove("complete");
    assert!(serde_json::from_value::<GoPackageInventoryOutcome>(missing).is_err());
}

#[test]
fn policy_and_scope_have_explicit_wire_names() {
    assert_eq!(
        serde_json::to_value(GoSelectionPolicy::Packages).unwrap(),
        "packages"
    );
    assert_eq!(
        serde_json::to_value(GoPackageTestScope::LinkedBinary).unwrap(),
        "linked-binary"
    );
    assert!(serde_json::from_str::<GoPackageTestScope>("\"integration\"").is_err());
    assert!(
        serde_json::from_value::<GoPackageProvenance>(
            serde_json::json!({"kind":"unknown","directory":"/p"})
        )
        .is_err()
    );
}
