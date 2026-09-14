use std::path::Path;

const GENERATED_GO_SUFFIXES: &[&str] = &[".pb.go", "_grpc.pb.go"];
const GENERATED_GO_DIRECTORIES: &[&str] = &["thriftgo"];
const TEST_GO_DIRECTORIES: &[&str] = &["test", "tests", "testfixture", "testfixtures"];
const TEST_GO_SUFFIX: &str = "_test.go";

pub(super) fn is_generated(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if GENERATED_GO_SUFFIXES
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
    {
        return true;
    }
    if file_name.ends_with(TEST_GO_SUFFIX) {
        return true;
    }

    path.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|value| {
            GENERATED_GO_DIRECTORIES.contains(&value) || TEST_GO_DIRECTORIES.contains(&value)
        })
    })
}
