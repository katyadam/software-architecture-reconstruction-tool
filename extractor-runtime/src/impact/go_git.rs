use super::go::GoImpactError;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug)]
pub struct GoHunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
}
#[derive(Debug)]
pub struct GoFileDiff {
    pub status: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub old_source: Option<Vec<u8>>,
    pub new_source: Option<Vec<u8>>,
    pub hunks: Vec<GoHunk>,
}
#[derive(Debug)]
pub struct GoGitDiff {
    pub module_root: PathBuf,
    pub baseline_revision: String,
    pub candidate_revision: String,
    pub files: Vec<GoFileDiff>,
}
pub(super) fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, GoImpactError> {
    let out = Command::new("git")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| GoImpactError::Git(e.to_string()))?;
    if !out.status.success() {
        return Err(GoImpactError::Git(
            String::from_utf8_lossy(&out.stderr).trim().into(),
        ));
    }
    Ok(out.stdout)
}
fn utf8(data: Vec<u8>) -> Result<String, GoImpactError> {
    String::from_utf8(data)
        .map_err(|_| GoImpactError::Input("non-UTF8 Git path or revision output".into()))
}
pub(super) fn revision(root: &Path, rev: &str) -> Result<String, GoImpactError> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err(GoImpactError::Input(
            "revision must not be empty or a Git option".into(),
        ));
    }
    Ok(utf8(git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )?)?
    .trim()
    .into())
}
pub(super) fn check_clean(root: &Path) -> Result<(), GoImpactError> {
    if !git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?
    .is_empty()
    {
        return Err(GoImpactError::Input(
            "candidate checkout is dirty (including untracked files)".into(),
        ));
    }
    Ok(())
}
fn check_scope(root: &Path) -> Result<(), GoImpactError> {
    if !root.join("go.mod").is_file() {
        return Err(GoImpactError::Input(
            "project-dir must contain go.mod".into(),
        ));
    }
    if std::env::var("GOWORK").is_ok_and(|v| !v.is_empty() && v != "off")
        || root.ancestors().any(|p| p.join("go.work").exists())
    {
        return Err(GoImpactError::Input(
            "active Go workspaces are unsupported; use an isolated single module".into(),
        ));
    }
    if std::env::var("GOFLAGS").is_ok_and(|v| !v.trim().is_empty()) {
        return Err(GoImpactError::Input(
            "ambient GOFLAGS unsupported; use explicit build tags".into(),
        ));
    }
    fn visit(root: &Path, at: &Path) -> Result<(), GoImpactError> {
        for item in fs::read_dir(at).map_err(|e| GoImpactError::Input(e.to_string()))? {
            let item = item.map_err(|e| GoImpactError::Input(e.to_string()))?;
            let ty = item
                .file_type()
                .map_err(|e| GoImpactError::Input(e.to_string()))?;
            if ty.is_dir() {
                if [".git", "vendor"].iter().any(|n| item.file_name() == *n) {
                    continue;
                }
                visit(root, &item.path())?
            } else if item.file_name() == "go.mod" && at != root {
                return Err(GoImpactError::Input(format!(
                    "nested Go module unsupported: {}",
                    item.path().display()
                )));
            }
        }
        Ok(())
    }
    visit(root, root)
}
fn count(value: &str) -> Result<(u32, u32), GoImpactError> {
    let (s, c) = value.split_once(',').unwrap_or((value, "1"));
    let start = s
        .parse::<u32>()
        .map_err(|_| GoImpactError::Git("malformed hunk start".into()))?;
    let count = c
        .parse::<u32>()
        .map_err(|_| GoImpactError::Git("malformed hunk count".into()))?;
    if start.checked_add(count).is_none() {
        return Err(GoImpactError::Git("hunk range overflow".into()));
    }
    Ok((start, count))
}
pub fn read_go_diff(root: &Path, baseline: &str) -> Result<GoGitDiff, GoImpactError> {
    let root = root
        .canonicalize()
        .map_err(|e| GoImpactError::Input(e.to_string()))?;
    let top = utf8(git(&root, &["rev-parse", "--show-toplevel"])?)?;
    if Path::new(top.trim()).canonicalize().ok().as_deref() != Some(&root) {
        return Err(GoImpactError::Input(
            "project-dir must be the Git/module root".into(),
        ));
    }
    let baseline = revision(&root, baseline)?;
    let candidate = revision(&root, "HEAD")?;
    check_clean(&root)?;
    check_scope(&root)?;
    let data = git(
        &root,
        &[
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            "--no-ext-diff",
            "--no-textconv",
            &baseline,
            &candidate,
            "--",
        ],
    )?;
    let mut fields = data.split(|b| *b == 0).peekable();
    let mut files = vec![];
    while let Some(raw) = fields.next() {
        if raw.is_empty() {
            if fields.peek().is_some() {
                return Err(GoImpactError::Git("empty status record".into()));
            }
            break;
        }
        let status = utf8(raw.to_vec())?;
        let kind = status.chars().next().unwrap();
        if !"MADRTC".contains(kind) {
            return Err(GoImpactError::Git(format!("unsupported status: {status}")));
        }
        let mut path = || -> Result<String, GoImpactError> {
            let p = fields
                .next()
                .ok_or_else(|| GoImpactError::Git("missing status path".into()))?;
            if p.is_empty() {
                return Err(GoImpactError::Git("empty status path".into()));
            }
            utf8(p.to_vec())
        };
        let first = path()?;
        let (old, new) = match kind {
            'A' => (None, Some(first)),
            'D' => (Some(first), None),
            'R' | 'C' => (Some(first), Some(path()?)),
            _ => (Some(first.clone()), Some(first)),
        };
        let old_source = old
            .as_ref()
            .map(|p| git(&root, &["cat-file", "blob", &format!("{baseline}:{p}")]))
            .transpose()?;
        let new_source = new
            .as_ref()
            .map(|p| git(&root, &["cat-file", "blob", &format!("{candidate}:{p}")]))
            .transpose()?;
        let mut hunks = vec![];
        if kind == 'M' {
            let p = new.as_deref().unwrap();
            let patch = git(
                &root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--unified=0",
                    &baseline,
                    &candidate,
                    "--",
                    p,
                ],
            )?;
            for line in String::from_utf8_lossy(&patch)
                .lines()
                .filter(|l| l.starts_with("@@ "))
            {
                let parts: Vec<_> = line.split(' ').collect();
                let a = parts
                    .get(1)
                    .and_then(|s| s.strip_prefix('-'))
                    .ok_or_else(|| GoImpactError::Git("malformed hunk".into()))?;
                let b = parts
                    .get(2)
                    .and_then(|s| s.strip_prefix('+'))
                    .ok_or_else(|| GoImpactError::Git("malformed hunk".into()))?;
                let (os, oc) = count(a)?;
                let (ns, nc) = count(b)?;
                hunks.push(GoHunk {
                    old_start: os,
                    old_count: oc,
                    new_start: ns,
                    new_count: nc,
                });
            }
        }
        files.push(GoFileDiff {
            status,
            old_path: old,
            new_path: new,
            old_source,
            new_source,
            hunks,
        });
    }
    Ok(GoGitDiff {
        module_root: root,
        baseline_revision: baseline,
        candidate_revision: candidate,
        files,
    })
}

/// Compare every active evidence file with the exact recorded candidate blob.
pub(super) fn verify_committed_sources(
    diff: &GoGitDiff,
    files: &[String],
) -> Result<(), GoImpactError> {
    for path in files {
        let rel = std::path::Path::new(path)
            .strip_prefix(&diff.module_root)
            .map_err(|_| GoImpactError::Input("inventory source outside module".into()))?;
        let relative = rel
            .to_str()
            .ok_or_else(|| GoImpactError::Input("non-UTF8 active Go source".into()))?;
        let committed = git(
            &diff.module_root,
            &[
                "cat-file",
                "blob",
                &format!("{}:{relative}", diff.candidate_revision),
            ],
        )
        .map_err(|_| {
            GoImpactError::Input(format!(
                "active Go source is not in candidate commit: {relative}"
            ))
        })?;
        let actual = std::fs::read(path).map_err(|e| GoImpactError::Input(e.to_string()))?;
        if committed != actual {
            return Err(GoImpactError::Input(format!(
                "active source differs from committed HEAD: {relative}"
            )));
        }
    }

    Ok(())
}
