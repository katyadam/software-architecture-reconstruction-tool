use super::registry::CallGraphProvider;
use models::{
    JavaTestCase,
    call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language},
};
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

    /// Analyzes a Maven module from discovered JUnit methods instead of only Java main methods.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(source_root.to_string_lossy(), Language::Java);
        self.invoke(&request, test_root_arguments(source_root, test_roots))
    }

    /// Launches the adapter with the supplied fully formed argument vector.
    fn invoke(&self, request: &CallGraphRequest, arguments: Vec<String>) -> CallGraphOutcome {
        let child = Command::new("java")
            .arg("-jar")
            .arg(&self.jar)
            .args(arguments)
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
impl CallGraphProvider for WalaJavaProvider {
    /// Declares that this adapter accepts Java source code.
    fn language(&self) -> Language {
        Language::Java
    }
    /// Executes WALA and validates its versioned JSON response before returning it.
    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.invoke(
            request,
            vec!["--source-dir".to_owned(), request.source_root.clone()],
        )
    }
}

/// Builds the repeatable adapter entrypoint arguments for discovered JUnit methods.
fn test_root_arguments(source_root: &std::path::Path, test_roots: &[JavaTestCase]) -> Vec<String> {
    let mut arguments = vec![
        "--source-dir".to_owned(),
        source_root.to_string_lossy().into_owned(),
    ];
    for test in test_roots {
        arguments.push("--entrypoint".to_owned());
        arguments.push(format!(
            "{}#{}{}",
            test.class_name, test.method_name, test.descriptor
        ));
    }
    arguments
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
    use super::{test_root_arguments, wait_for_output};
    use models::JavaTestCase;
    use std::{path::Path, process::Command, time::Duration};

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

    #[test]
    fn serializes_each_test_root_as_an_adapter_entrypoint() {
        let tests = vec![JavaTestCase {
            module_root: "module".into(),
            class_name: "Lexample/ServiceTest".into(),
            method_name: "changesService".into(),
            descriptor: "()V".into(),
            callable_signature: "ServiceTest.changesService()V".into(),
            source_path: "src/test/java/example/ServiceTest.java".into(),
        }];

        assert_eq!(
            test_root_arguments(Path::new("module/src/main/java"), &tests),
            vec![
                "--source-dir".to_owned(),
                "module/src/main/java".to_owned(),
                "--entrypoint".to_owned(),
                "Lexample/ServiceTest#changesService()V".to_owned(),
            ]
        );
    }
}
