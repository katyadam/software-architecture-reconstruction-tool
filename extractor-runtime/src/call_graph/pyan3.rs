use std::{
    collections::HashMap,
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

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
            return Pyan3Result {
                status: Pyan3Status::Ok,
                edges: HashMap::new(),
                diagnostics: vec!["no Python source files found".into()],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
        let mut cmd = Command::new("pyan3");
        cmd.args(&files)
            .args(["--text", "--uses", "--no-defines"])
            .current_dir(source_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Pyan3Result {
                    status: Pyan3Status::NotInstalled,
                    edges: HashMap::new(),
                    diagnostics: vec![format!("pyan3 not found in PATH: {e}")],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(e) => {
                return Pyan3Result {
                    status: Pyan3Status::Failed,
                    edges: HashMap::new(),
                    diagnostics: vec![format!("failed to start pyan3: {e}")],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };
        // Wait with timeout via a thread.
        let output = wait_with_timeout(child, self.timeout);
        let duration_ms = start.elapsed().as_millis() as u64;
        match output {
            Err(e) => Pyan3Result {
                status: Pyan3Status::Failed,
                edges: HashMap::new(),
                diagnostics: vec![e],
                duration_ms,
            },
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if !out.status.success() {
                    return Pyan3Result {
                        status: Pyan3Status::Failed,
                        edges: HashMap::new(),
                        diagnostics: vec![format!(
                            "pyan3 exited {}: {}",
                            out.status,
                            tail(&stderr)
                        )],
                        duration_ms,
                    };
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

/// Collects all `.py` files under `root`, skipping `.git`, `__pycache__`, and hidden dirs.
fn collect_py_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_recursive(root, root, &mut files);
    files.sort();
    files
}

fn collect_recursive(root: &Path, dir: &Path, out: &mut Vec<String>) {
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
            collect_recursive(root, &path, out);
        } else if name.ends_with(".py") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
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

fn skipped(msg: &str) -> Pyan3Result {
    Pyan3Result {
        status: Pyan3Status::Ok,
        edges: HashMap::new(),
        diagnostics: vec![msg.into()],
        duration_ms: 0,
    }
}

fn tail(text: &str) -> &str {
    let start = text.len().saturating_sub(2048);
    let mut s = start;
    while s < text.len() && !text.is_char_boundary(s) {
        s += 1;
    }
    &text[s..]
}

/// Waits for a child process, killing it if the timeout elapses.
fn wait_with_timeout(
    child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    use std::{sync::mpsc, thread};
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("pyan3 I/O error: {e}")),
        Err(_) => Err(format!("pyan3 timed out after {}s", timeout.as_secs())),
    }
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
