use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use super::adapter_process;

pub enum Pyan3Status {
    Ok,
    NotInstalled,
    Failed,
}

pub struct Pyan3Result {
    pub status: Pyan3Status,
    /// caller → list of callees (dotted qualified names with module prefix, e.g. `pkg.Class.method`).
    pub edges: HashMap<String, Vec<String>>,
    pub diagnostics: Vec<String>,
    pub duration_ms: u64,
}

pub struct Pyan3Provider {
    timeout: Duration,
}

impl Pyan3Provider {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub fn analyze_project(&self, source_root: &Path) -> Pyan3Result {
        if self.timeout.is_zero() {
            return skipped("pyan3 analysis skipped (timeout=0)");
        }
        let start = Instant::now();
        let files = collect_py_files(source_root);
        if files.is_empty() {
            return early(
                Pyan3Status::Ok,
                "no Python source files found",
                start.elapsed().as_millis() as u64,
            );
        }
        let mut cmd = Command::new("pyan3");
        cmd.args(&files)
            .args(["--text", "--uses", "--no-defines"])
            .current_dir(source_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        adapter_process::isolate_process_group(&mut cmd);
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return early(
                    Pyan3Status::NotInstalled,
                    format!("pyan3 not found in PATH: {e}"),
                    start.elapsed().as_millis() as u64,
                );
            }
            Err(e) => {
                return early(
                    Pyan3Status::Failed,
                    format!("failed to start pyan3: {e}"),
                    start.elapsed().as_millis() as u64,
                );
            }
        };
        let output = adapter_process::wait_for_output(child, self.timeout, "pyan3");
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Err(e) => early(Pyan3Status::Failed, e, duration_ms),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if !out.status.success() {
                    return early(
                        Pyan3Status::Failed,
                        format!("pyan3 exited {}: {}", out.status, tail(&stderr)),
                        duration_ms,
                    );
                }
                let text = String::from_utf8_lossy(&out.stdout);
                let mut diag = vec![];
                if !stderr.trim().is_empty() {
                    diag.push(format!("pyan3 stderr: {}", tail(&stderr)));
                }
                let edges = parse_text_output(&text);
                Pyan3Result {
                    status: Pyan3Status::Ok,
                    edges,
                    diagnostics: diag,
                    duration_ms,
                }
            }
        }
    }
}

/// Collects all `.py` files under `root` as relative paths (POSIX separators),
/// ready to pass as arguments to the `pyan3` CLI.
fn collect_py_files(root: &Path) -> Vec<String> {
    crate::py_walk::collect_py_paths(root)
        .into_iter()
        .filter_map(|p| {
            p.strip_prefix(root)
                .ok()
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

/// Parses pyan3 `--text` output into a caller → callees adjacency map.
///
/// Format:
/// ```text
/// module.ClassName.method
///  [U] other_module.OtherClass.other_method
///  [D] module.ClassName.inner
/// ```
/// We only collect `[U]` (uses/calls) edges.
pub fn parse_text_output(text: &str) -> HashMap<String, Vec<String>> {
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            let trimmed = line.trim();
            if let Some(callee) = trimmed.strip_prefix("[U] ") {
                if let Some(caller) = &current {
                    edges
                        .entry(caller.clone())
                        .or_default()
                        .push(callee.trim().to_owned());
                }
            }
            // [D] lines (defines) are skipped.
        } else if !line.trim().is_empty() {
            current = Some(line.trim().to_owned());
            edges.entry(line.trim().to_owned()).or_default();
        }
    }
    edges
}

fn early(status: Pyan3Status, msg: impl Into<String>, duration_ms: u64) -> Pyan3Result {
    Pyan3Result {
        status,
        edges: HashMap::new(),
        diagnostics: vec![msg.into()],
        duration_ms,
    }
}

fn skipped(msg: &str) -> Pyan3Result {
    early(Pyan3Status::Ok, msg, 0)
}

fn tail(text: &str) -> &str {
    let start = text.len().saturating_sub(2048);
    let mut s = start;
    while s < text.len() && !text.is_char_boundary(s) {
        s += 1;
    }
    &text[s..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_text_output_extracts_use_edges() {
        let text = "mymod.MyClass.method\n [U] other.OtherClass.other\n [D] mymod.MyClass.inner\nother.OtherClass.other\n";
        let edges = parse_text_output(text);
        // The caller node must be present.
        assert!(edges.contains_key("mymod.MyClass.method"), "{edges:?}");
        // [U] edge recorded.
        assert_eq!(
            edges["mymod.MyClass.method"],
            vec!["other.OtherClass.other"]
        );
        // [D] line is NOT recorded as an edge.
        assert!(!edges["mymod.MyClass.method"].contains(&"mymod.MyClass.inner".to_string()));
        // Callee node also appears (it has no outgoing edges here).
        assert!(edges.contains_key("other.OtherClass.other"));
        assert!(edges["other.OtherClass.other"].is_empty());
    }

    #[test]
    fn parse_text_output_empty_input() {
        assert!(parse_text_output("").is_empty());
    }
}
