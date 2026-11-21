//! Project file walk and Pass 1 over it, shared by the CLIs.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use models::ir::syntax::FileRecord;

use super::dispatch_syntactic;

/// Every file below `dir`, skipping `.git` and `target`, sorted so the order
/// does not depend on the filesystem.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = walk(dir)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                continue;
            }
            if path.is_dir() {
                out.extend(walk(&path)?);
            } else {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// Pass 1 on each path; unreadable files are skipped with a warning.
pub fn collect_syntactic_records(
    paths: Vec<PathBuf>,
    report_progress: bool,
) -> Result<Vec<FileRecord>> {
    let total = paths.len();
    if report_progress {
        println!("Total files found: {total}");
    }
    let mut records = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let code = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "Skipping file {:?}: {e}",
                    path.file_name().unwrap_or_default()
                );
                continue;
            }
        };
        if report_progress {
            println!(
                "Extracting ({}/{total}): {:?}",
                i + 1,
                path.file_name().unwrap_or_default()
            );
        }
        if let Some(record) = dispatch_syntactic(&code, path.to_str().unwrap_or_default())
            .with_context(|| format!("Error dispatching file: {path:?}"))?
        {
            records.push(record);
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_sorted_and_skips_git() {
        let dir = tempfile::tempdir().expect("tempdir");
        for f in ["b.py", "a/c.py", ".git/x", "target/y"] {
            let p = dir.path().join(f);
            fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
            fs::write(&p, "").expect("write");
        }
        let got: Vec<_> = collect_files(dir.path())
            .expect("walks")
            .into_iter()
            .map(|p| p.strip_prefix(dir.path()).expect("prefix").to_path_buf())
            .collect();
        assert_eq!(got, vec![PathBuf::from("a/c.py"), PathBuf::from("b.py")]);
    }
}
