use models::{go_impact::GoInventoryOutcome, go_package_impact::GoPackageInventoryOutcome};
use std::{fs, path::Path};

pub fn fixture(root: &Path) -> (GoInventoryOutcome, GoPackageInventoryOutcome) {
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(
        root.join("a/a.go"),
        "package a; func Value() int { return 1 }",
    )
    .unwrap();
    fs::write(
        root.join("a/a_test.go"),
        "package a; import \"testing\"; func TestA(t *testing.T) {}",
    )
    .unwrap();
    let root = root.canonicalize().unwrap();
    let source = root.join("a/a.go").to_string_lossy().into_owned();
    let test = root.join("a/a_test.go").to_string_lossy().into_owned();
    let scope = serde_json::json!({"dependency_bodies":"opaque","goos":"linux","goarch":"amd64","go_version":"go1.26.0","build_tags":"","workspace_mode":"off","limitations":["opaque dependencies"]});
    let inventory = serde_json::from_value(serde_json::json!({"schema_version":1,"provider_id":"go-native","status":"ok","source_root":root,"source_files":[source,test],"tests":[{"method":{"language":"go","declaring_type":"example/a","member_name":"TestA","descriptor":"func(t *testing.T)","source_path":test,"source_line":1},"execution_package":"example/a"}],"declarations":[],"test_inventory_complete":true,"scope":scope,"diagnostics":[]})).unwrap();
    let provenance = serde_json::json!({"kind":"verified_module","directory":root.join("a"),"module_path":"example"});
    let evidence = serde_json::from_value(serde_json::json!({"schema_version":1,"provider_id":"go-native","status":"ok","source_root":root,"scope":scope,"complete":true,"effective_build_context":{"go_version":"go1.26.0","goos":"linux","goarch":"amd64","cgo_enabled":"0","architecture_features":{"GOAMD64":"v1"},"goexperiment":"","build_tags":"","environment":{"GOENV":"off","GOWORK":"off","GOTOOLCHAIN":"local","GOPROXY":"off","GOSUMDB":"off","GOFLAGS":"","GOOS":"linux","GOARCH":"amd64","CGO_ENABLED":"0","GOEXPERIMENT":"","GOAMD64":"v1"},"build_flags":["-mod=readonly"]},"packages":[{"id":"production-opaque","import_path":"example/a","imports":[],"provenance":provenance,"source_files":[source],"test_files":[],"for_test":""},{"id":"binary-opaque","import_path":"example/a","imports":[],"provenance":provenance,"source_files":[source,test],"test_files":[test],"for_test":"example/a"}],"execution_packages":[{"import_path":"example/a","relative_directory":"a","roots":["binary-opaque"]}],"blockers":[],"diagnostics":[]})).unwrap();
    (inventory, evidence)
}
