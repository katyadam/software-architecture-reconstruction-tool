use std::{
    fs,
    path::{Path, PathBuf},
};

/// Returns absolute paths of every `.py` file under `root`, sorted lexicographically.
///
/// Skips directories whose names start with `.` (covering `.git`, `.venv`, etc.)
/// and `__pycache__`.
pub(crate) fn collect_py_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    walk(root, &mut paths);
    paths.sort();
    paths
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        if path.is_dir() {
            if name.starts_with('.') || name == "__pycache__" {
                continue;
            }
            walk(&path, out);
        } else if name.ends_with(".py") {
            out.push(path);
        }
    }
}
