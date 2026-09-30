use super::registry::CallGraphProvider;
use models::call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language};
use std::{
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(300);

/// Invokes the standalone WALA adapter for Java source trees.
pub struct WalaJavaProvider {
    jar: PathBuf,
    timeout: Duration,
}
impl WalaJavaProvider {
    /// Creates a Java provider that launches the adapter JAR at `jar`.
    pub fn new(jar: impl Into<PathBuf>) -> Self {
        Self::with_timeout(jar, DEFAULT_ANALYSIS_TIMEOUT)
    }

    /// Creates a Java provider with a deadline that bounds one external WALA invocation.
    pub fn with_timeout(jar: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            jar: jar.into(),
            timeout,
        }
    }
}
impl CallGraphProvider for WalaJavaProvider {
    /// Declares that this adapter accepts Java source code.
    fn language(&self) -> Language {
        Language::Java
    }
    /// Executes WALA and validates its versioned JSON response before returning it.
    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        let child = Command::new("java")
            .args([
                "-jar",
                self.jar.to_string_lossy().as_ref(),
                "--source-dir",
                &request.source_root,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let output = child
            .and_then(|child| wait_for_output(child, self.timeout).map_err(std::io::Error::other));
        match output {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<CallGraphOutcome>(&output.stdout) {
                    Ok(result)
                        if result.schema_version == 1
                            && result.status != CallGraphStatus::Failed =>
                    {
                        result
                    }
                    Ok(_) | Err(_) => failed(request, "WALA produced invalid call-graph JSON"),
                }
            }
            Ok(output) => failed(request, &String::from_utf8_lossy(&output.stderr)),
            Err(error) => failed(request, &error.to_string()),
        }
    }
}

/// Waits for an adapter process and terminates it when it exceeds the configured deadline.
fn wait_for_output(mut child: Child, timeout: Duration) -> Result<Output, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|error| error.to_string()),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("WALA analysis timed out after {timeout:?}"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Converts adapter process or protocol failures into a non-fatal provider outcome.
fn failed(request: &CallGraphRequest, diagnostic: &str) -> CallGraphOutcome {
    CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Failed,
        provider_id: "wala-java".into(),
        source_root: request.source_root.clone(),
        algorithm: "cha_bytecode".into(),
        diagnostics: vec![diagnostic.into()],
        edges: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::wait_for_output;
    use std::{process::Command, time::Duration};

    #[test]
    fn stops_a_child_process_that_exceeds_the_deadline() {
        let child = Command::new("sleep")
            .arg("1")
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(10))
            .expect_err("a process that sleeps longer than its deadline must time out");

        assert!(error.contains("timed out"), "{error}");
    }
}
