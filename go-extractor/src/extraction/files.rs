use std::path::Path;

const GENERATED_GO_SUFFIXES: &[&str] = &[".pb.go", "_grpc.pb.go"];
const GENERATED_GO_DIRECTORIES: &[&str] = &["thriftgo"];

pub(super) fn should_extract(path: &Path) -> bool {
    !is_generated(path)
}

fn is_generated(path: &Path) -> bool {
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

    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|value| GENERATED_GO_DIRECTORIES.contains(&value))
    })
}
