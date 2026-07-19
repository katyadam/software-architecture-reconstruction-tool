use cli::metadata::next_run_number;
use std::fs;
use std::path::PathBuf;

fn write_meta(dir: &std::path::Path, benchmark: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("run_metadata.json"),
        format!("{{\"benchmark\":{{\"name\":\"{benchmark}\"}}}}"),
    )
    .unwrap();
}

#[test]
fn next_run_number_counts_matching_benchmark() {
    let parent = std::env::temp_dir().join(format!("nrn_it_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&parent);

    write_meta(&parent.join("run-1"), "foo");
    write_meta(&parent.join("run-2"), "foo");
    write_meta(&parent.join("run-3"), "bar");
    // A directory with no metadata should be ignored.
    fs::create_dir_all(parent.join("empty")).unwrap();

    let new_out = parent.join("run-4"); // does not exist yet
    assert_eq!(next_run_number(&parent, &new_out, "foo"), 3);
    assert_eq!(next_run_number(&parent, &new_out, "bar"), 2);
    // A benchmark never seen before starts at 1.
    assert_eq!(next_run_number(&parent, &new_out, "baz"), 1);
    // Empty parent -> 1.
    let empty_parent = PathBuf::from(parent.join("nonexistent"));
    assert_eq!(next_run_number(&empty_parent, &new_out, "foo"), 1);

    let _ = fs::remove_dir_all(&parent);
}
