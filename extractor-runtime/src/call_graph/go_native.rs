//! Typed boundary for the independent Go executable. This deliberately does
//! not use Java test identities, source resolution, or artifact preparation.

use super::{
    CallGraphProvider,
    adapter_process::{isolate_process_group, wait_for_output},
};
use models::call_graph::{
    CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language, MethodRef,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Duration,
};

const PROVIDER_ID: &str = "go-native";
const ALGORITHM: &str = "cha";

/// Native analysis context must survive deserialization: an opaque dependency
/// scope cannot be silently treated as a complete graph by a TIA consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoAnalysisScope {
    pub dependency_bodies: String,
    pub goos: String,
    pub goarch: String,
    pub go_version: String,
    pub build_tags: String,
    pub workspace_mode: String,
    pub limitations: Vec<String>,
}

/// A successful graph computation is distinct from complete exclusion coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoAnalysisResult {
    #[serde(flatten)]
    pub graph: CallGraphOutcome,
    pub tests: Vec<MethodRef>,
    pub scope: Option<GoAnalysisScope>,
}

impl GoAnalysisResult {
    /// The current CHA proof has no certified complete mode. Do not infer one
    /// from an empty limitations list or trust a future binary's scope claim.
    pub fn requires_conservative_fallback(&self) -> bool {
        true
    }

    /// The legacy generic contract has nowhere to carry scope/test inventory.
    /// Make loss of that context fail closed rather than expose partial paths
    /// as evidence for excluding tests.
    pub fn into_conservative_outcome(mut self) -> CallGraphOutcome {
        if !matches!(
            self.graph.status,
            CallGraphStatus::Failed | CallGraphStatus::Unsupported
        ) {
            self.graph.status = CallGraphStatus::Unsupported;
        }
        self.graph.diagnostics.push("Go native CHA proof is not certified for safe test exclusion; conservative fallback required".into());
        if let Some(scope) = self.scope {
            self.graph.diagnostics.push(format!(
                "Go dependency bodies={}; workspace={}; platform={}/{}; build tags={}",
                scope.dependency_bodies,
                scope.workspace_mode,
                scope.goos,
                scope.goarch,
                scope.build_tags
            ));
            self.graph.diagnostics.extend(
                scope
                    .limitations
                    .into_iter()
                    .map(|s| format!("Go coverage limitation: {s}")),
            );
        }
        self.graph.reachability.clear();
        self.graph.edges.clear();
        self.graph
    }
}

/// Launches a prebuilt native executable. Building/downloading tools is a
/// separate setup step, never an implicit side effect of graph analysis.
pub struct GoNativeProvider {
    executable: PathBuf,
    timeout: Duration,
    build_tags: String,
}

impl GoNativeProvider {
    pub fn new(executable: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            executable: executable.into(),
            timeout,
            build_tags: String::new(),
        }
    }

    pub fn with_build_tags(mut self, build_tags: impl Into<String>) -> Self {
        self.build_tags = build_tags.into();
        self
    }

    /// Targets use native package#Function or package#(Receiver).Method keys.
    /// No empty graph request is converted into an implicit "all methods" run.
    pub fn analyze_targets(&self, source_root: &Path, targets: &[String]) -> GoAnalysisResult {
        if targets.is_empty() {
            return failure(source_root, "Go native analysis requires explicit targets");
        }
        if self.timeout.is_zero() {
            return failure(
                source_root,
                "Go native analysis requires a positive timeout",
            );
        }
        if !cfg!(unix) {
            return failure(
                source_root,
                "Go native adapter process-tree supervision currently supports Unix only",
            );
        }
        let root = match source_root.canonicalize() {
            Ok(root) if root.is_dir() => root,
            _ => return failure(source_root, "Go source root must be an existing directory"),
        };
        if targets.iter().any(|key| {
            key.split_once('#')
                .is_none_or(|(pkg, name)| pkg.is_empty() || name.is_empty() || name.contains('#'))
        }) {
            return failure(
                &root,
                "Go targets must be native package#Function or package#(Receiver).Method identities",
            );
        }
        let mut command = Command::new(&self.executable);
        command
            .arg("--source-dir")
            .arg(&root)
            .arg("--timeout")
            .arg(duration_flag(self.timeout));
        // Several diff hunks can map to the same declaration. Ask for it once
        // while still rejecting unsolicited duplicate records in the response.
        let mut sent_targets = BTreeSet::new();
        for target in targets {
            if sent_targets.insert(target) {
                command.arg("--target").arg(target);
            }
        }
        if !self.build_tags.is_empty() {
            command.arg("--tags").arg(&self.build_tags);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_process_group(&mut command);
        match command.spawn() {
            Err(error) => failure(
                &root,
                &format!("Unable to start Go native adapter: {error}"),
            ),
            Ok(child) => match wait_for_output(child, self.timeout, PROVIDER_ID) {
                Err(error) => failure(&root, &error),
                Ok(output) => self.parse_output(&root, targets, output),
            },
        }
    }

    fn parse_output(&self, root: &Path, targets: &[String], output: Output) -> GoAnalysisResult {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut result: GoAnalysisResult = match serde_json::from_slice(&output.stdout) {
            Ok(result) => result,
            Err(error) => {
                return failure(
                    root,
                    &format!(
                        "Go native adapter produced invalid JSON ({error}); exit={}; stderr: {}",
                        output.status,
                        diagnostic_tail(&stderr)
                    ),
                );
            }
        };
        if let Err(error) = validate_response(&result, root, targets, &self.build_tags) {
            return failure(root, &format!("Go native adapter protocol error: {error}"));
        }
        if !output.status.success() && result.graph.status != CallGraphStatus::Failed {
            return failure(
                root,
                &format!(
                    "Go native adapter exited {} despite successful JSON; stderr: {}",
                    output.status,
                    diagnostic_tail(&stderr)
                ),
            );
        }
        if result.graph.status == CallGraphStatus::Failed {
            result.graph.reachability.clear();
            result.graph.edges.clear();
            if result.graph.diagnostics.is_empty() {
                result
                    .graph
                    .diagnostics
                    .push("Go native adapter reported failure".into());
            }
            if !stderr.trim().is_empty() {
                result
                    .graph
                    .diagnostics
                    .push(format!("Go adapter stderr: {}", diagnostic_tail(&stderr)));
            }
        }
        result
    }
}

impl CallGraphProvider for GoNativeProvider {
    fn language(&self) -> Language {
        Language::Go
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        let mut result = failure(
            Path::new(&request.source_root),
            "Go native analysis requires explicit targets and typed coverage; use analyze_targets",
        );
        result.graph.status = CallGraphStatus::Unsupported;
        result.into_conservative_outcome()
    }
}

fn duration_flag(timeout: Duration) -> String {
    if timeout.subsec_nanos() == 0 {
        format!("{}s", timeout.as_secs())
    } else {
        format!("{}ns", timeout.as_nanos())
    }
}

fn diagnostic_tail(text: &str) -> &str {
    let mut start = text.len().saturating_sub(4_096);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

fn failure(root: &Path, message: &str) -> GoAnalysisResult {
    let mut graph = CallGraphOutcome::ok(
        CallGraphRequest::new(root.to_string_lossy(), Language::Go),
        PROVIDER_ID,
        ALGORITHM,
    );
    graph.schema_version = 2;
    graph.status = CallGraphStatus::Failed;
    graph.diagnostics.push(message.into());
    GoAnalysisResult {
        graph,
        tests: vec![],
        scope: None,
    }
}

fn selector(method: &MethodRef) -> String {
    format!("{}#{}", method.declaring_type, method.member_name)
}

fn validate_response(
    result: &GoAnalysisResult,
    root: &Path,
    targets: &[String],
    build_tags: &str,
) -> Result<(), String> {
    let graph = &result.graph;
    if graph.schema_version != 2 || graph.provider_id != PROVIDER_ID || graph.algorithm != ALGORITHM
    {
        return Err("unexpected schema version, provider, or algorithm".into());
    }
    if Path::new(&graph.source_root).canonicalize().ok().as_deref() != Some(root) {
        return Err("source root does not match request".into());
    }
    let scope = result
        .scope
        .as_ref()
        .ok_or("missing native scope metadata")?;
    if scope.goos.is_empty()
        || scope.goarch.is_empty()
        || scope.go_version.is_empty()
        || scope.dependency_bodies.is_empty()
        || scope.workspace_mode != "off"
        || scope.build_tags != build_tags
    {
        return Err("invalid or mismatched native build scope".into());
    }
    let validate_ref = |method: &MethodRef| -> Result<(), String> {
        if method.language != Language::Go
            || method.declaring_type.is_empty()
            || method.member_name.is_empty()
            || !method.descriptor.starts_with("func")
            || method.source_line.is_none_or(|line| line == 0)
        {
            return Err("missing or non-Go source identity".into());
        }
        let path = method
            .source_path
            .as_ref()
            .and_then(|path| Path::new(path).canonicalize().ok())
            .ok_or("source reference has no existing local file")?;
        if !path.starts_with(root) || !path.is_file() {
            return Err("source reference escapes the module".into());
        }
        Ok(())
    };
    let mut inventory: Vec<&MethodRef> = Vec::new();
    for test in &result.tests {
        validate_ref(test)?;
        if inventory.contains(&test) {
            return Err("duplicate native test declaration".into());
        }
        inventory.push(test);
    }
    if !graph.edges.is_empty() {
        return Err("targeted Go proof must not supply unvalidated raw edges".into());
    }
    if graph.status == CallGraphStatus::Failed {
        return Ok(());
    }
    if !matches!(
        graph.status,
        CallGraphStatus::Ok | CallGraphStatus::NoEntrypoints
    ) {
        return Err("unexpected Go analysis status".into());
    }
    if (graph.status == CallGraphStatus::NoEntrypoints) != result.tests.is_empty() {
        return Err("status does not match native test inventory".into());
    }
    let requested: BTreeSet<_> = targets.iter().cloned().collect();
    let mut returned = BTreeSet::new();
    for record in &graph.reachability {
        validate_ref(&record.method)?;
        let key = selector(&record.method);
        if !requested.contains(&key) || !returned.insert(key) {
            return Err("unexpected or duplicate native target".into());
        }
        for test in &record.entrypoints {
            if !inventory.contains(&test) {
                return Err("reachability contains an undiscovered test".into());
            }
        }
    }
    if returned != requested {
        return Err(
            "missing requested target reachability (empty is not equivalent to missing)".into(),
        );
    }
    Ok(())
}
