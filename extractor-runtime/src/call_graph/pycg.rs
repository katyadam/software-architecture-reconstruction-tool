use std::{
    collections::HashMap,
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub enum PyCGStatus {
    Ok,
    NotInstalled,
    Failed,
}

pub struct PyCGResult {
    pub status: PyCGStatus,
    /// caller -> list of callees (dotted qualified names, e.g. `pkg.Class.method`).
    pub edges: HashMap<String, Vec<String>>,
    pub diagnostics: Vec<String>,
    pub duration_ms: u64,
}

pub struct PyCGProvider {
    timeout: Duration,
}

impl PyCGProvider {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub fn analyze_project(&self, source_root: &Path) -> PyCGResult {
        if self.timeout.is_zero() {
            return skipped("pycg analysis skipped (timeout=0)");
        }
        let start = Instant::now();
        let files = collect_py_files(source_root);
        if files.is_empty() {
            return PyCGResult {
                status: PyCGStatus::Ok,
                edges: HashMap::new(),
                diagnostics: vec!["no Python source files found".into()],
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // PyCG writes JSON to a file rather than stdout; use a unique temp path.
        let tmp_path = std::env::temp_dir()
            .join(format!("pycg-{}.json", uuid::Uuid::new_v4()));

        let mut cmd = Command::new("pycg");
        cmd.args(&files);
        // Add --package when the root is a Python package (has __init__.py).
        if source_root.join("__init__.py").exists() {
            if let Some(name) = source_root.file_name().and_then(|n| n.to_str()) {
                cmd.arg("--package").arg(name);
            }
        }
        cmd.arg("-o")
            .arg(&tmp_path)
            .current_dir(source_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return PyCGResult {
                    status: PyCGStatus::NotInstalled,
                    edges: HashMap::new(),
                    diagnostics: vec![format!("pycg not found in PATH: {e}")],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(e) => {
                return PyCGResult {
                    status: PyCGStatus::Failed,
                    edges: HashMap::new(),
                    diagnostics: vec![format!("failed to start pycg: {e}")],
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        let output = wait_with_timeout(child, self.timeout);
        let duration_ms = start.elapsed().as_millis() as u64;

        let out = match output {
            Err(e) => {
                let _ = fs::remove_file(&tmp_path);
                return PyCGResult {
                    status: PyCGStatus::Failed,
                    edges: HashMap::new(),
                    diagnostics: vec![e],
                    duration_ms,
                };
            }
            Ok(o) => o,
        };

        let stderr = String::from_utf8_lossy(&out.stderr);
        if !out.status.success() {
            let _ = fs::remove_file(&tmp_path);
            return PyCGResult {
                status: PyCGStatus::Failed,
                edges: HashMap::new(),
                diagnostics: vec![format!("pycg exited {}: {}", out.status, tail(&stderr))],
                duration_ms,
            };
        }

        let json_text = match fs::read_to_string(&tmp_path) {
            Ok(t) => {
                let _ = fs::remove_file(&tmp_path);
                t
            }
            Err(e) => {
                return PyCGResult {
                    status: PyCGStatus::Failed,
                    edges: HashMap::new(),
                    diagnostics: vec![format!("failed to read pycg output: {e}")],
                    duration_ms,
                };
            }
        };

        let mut diag = vec![];
        if !stderr.trim().is_empty() {
            diag.push(format!("pycg stderr: {}", tail(&stderr)));
        }
        let edges = parse_pycg_output(&json_text);
        PyCGResult {
            status: PyCGStatus::Ok,
            edges,
            diagnostics: diag,
            duration_ms,
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

/// Parses PyCG JSON output into a caller -> callees adjacency map.
///
/// Handles two formats:
/// - **pycg 2.0.0** (`format: "pycg/1"`): `{"calls": {"caller": [{"callees": [...], ...}], ...}, ...}`
/// - **legacy flat**: `{"caller.dotted.name": ["callee1", "callee2"], ...}`
///
/// Every callee is also inserted as a key (possibly empty) so the BFS graph is complete.
pub fn parse_pycg_output(text: &str) -> HashMap<String, Vec<String>> {
    let root: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    // pycg 2.0.0: top-level has a "calls" object; each value is a list of call-site objects.
    if let Some(calls) = root.get("calls").and_then(|c| c.as_object()) {
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        for (caller, call_sites) in calls {
            let callees: Vec<String> = call_sites
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|site| {
                    site.get("callees")
                        .and_then(|c| c.as_array())
                        .into_iter()
                        .flatten()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                })
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            for callee in &callees {
                edges.entry(callee.clone()).or_default();
            }
            edges.insert(caller.clone(), callees);
        }
        return edges;
    }

    // Legacy flat format: {"caller": ["callee1", ...], ...}
    let flat: HashMap<String, Vec<String>> = match serde_json::from_value(root) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for (caller, callees) in flat {
        for callee in &callees {
            edges.entry(callee.clone()).or_default();
        }
        edges.insert(caller, callees);
    }
    edges
}

fn skipped(msg: &str) -> PyCGResult {
    PyCGResult {
        status: PyCGStatus::Ok,
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
        Ok(Err(e)) => Err(format!("pycg I/O error: {e}")),
        Err(_) => Err(format!("pycg timed out after {}s", timeout.as_secs())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // pycg 2.0.0 format tests
    #[test]
    fn parse_pycg_v2_extracts_edges() {
        let json = r#"{"calls": {"mymod.MyClass.method": [{"callees": ["other.OtherClass.other"], "col": 4, "line": 5}]}, "format": "pycg/1"}"#;
        let edges = parse_pycg_output(json);
        assert!(edges.contains_key("mymod.MyClass.method"), "{edges:?}");
        assert_eq!(edges["mymod.MyClass.method"], vec!["other.OtherClass.other"]);
        assert!(edges.contains_key("other.OtherClass.other"), "callee must be a key");
        assert!(edges["other.OtherClass.other"].is_empty());
    }

    #[test]
    fn parse_pycg_v2_multiple_call_sites_deduplicated() {
        // Same callee referenced from two call sites → deduplicated to one edge entry.
        let json = r#"{"calls": {"a.b": [{"callees": ["c.d"], "line": 1}, {"callees": ["c.d", "e.f"], "line": 2}]}, "format": "pycg/1"}"#;
        let edges = parse_pycg_output(json);
        assert!(edges.contains_key("c.d"), "c.d must be a key");
        assert!(edges.contains_key("e.f"), "e.f must be a key");
        let callees = &edges["a.b"];
        // c.d appears twice in raw sites — deduplication means it appears exactly once.
        assert_eq!(callees.iter().filter(|s| s.as_str() == "c.d").count(), 1);
    }

    #[test]
    fn parse_pycg_v2_empty_calls() {
        let json = r#"{"calls": {}, "format": "pycg/1"}"#;
        assert!(parse_pycg_output(json).is_empty());
    }

    // Legacy flat format tests (backward compatibility)
    #[test]
    fn parse_pycg_legacy_extracts_edges() {
        let json = r#"{"mymod.MyClass.method": ["other.OtherClass.other"], "other.OtherClass.other": []}"#;
        let edges = parse_pycg_output(json);
        assert!(edges.contains_key("mymod.MyClass.method"), "{edges:?}");
        assert_eq!(edges["mymod.MyClass.method"], vec!["other.OtherClass.other"]);
        assert!(edges.contains_key("other.OtherClass.other"));
        assert!(edges["other.OtherClass.other"].is_empty());
    }

    #[test]
    fn parse_pycg_legacy_adds_callee_nodes() {
        let json = r#"{"a.b": ["c.d", "e.f"]}"#;
        let edges = parse_pycg_output(json);
        assert!(edges.contains_key("c.d"), "callee c.d must be a key");
        assert!(edges.contains_key("e.f"), "callee e.f must be a key");
        assert!(edges["c.d"].is_empty());
        assert!(edges["e.f"].is_empty());
    }

    #[test]
    fn parse_pycg_output_empty_input() {
        assert!(parse_pycg_output("").is_empty());
        assert!(parse_pycg_output("{}").is_empty());
    }
}
