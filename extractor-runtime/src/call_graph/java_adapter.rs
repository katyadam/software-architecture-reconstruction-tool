use super::registry::CallGraphProvider;
use models::{
    JavaTestCase,
    call_graph::{CallGraphOutcome, CallGraphRequest, CallGraphStatus, Language},
};
use std::{
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{Child, ChildStderr, ChildStdout, Command, Output, Stdio},
    sync::mpsc,
    time::Duration,
};

#[cfg(not(unix))]
use std::{thread, time::Instant};

pub const DEFAULT_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(300);

/// Launches a standalone Java call-graph adapter using the versioned JSON protocol.
pub struct JavaAdapterProvider {
    provider_id: String,
    jar: PathBuf,
    timeout: Duration,
    algorithm: String,
}

impl JavaAdapterProvider {
    pub fn new(
        provider_id: impl Into<String>,
        jar: impl Into<PathBuf>,
        timeout: Duration,
        algorithm: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            jar: jar.into(),
            timeout,
            algorithm: algorithm.into(),
        }
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Analyzes one module from discovered JUnit methods and changed callable targets.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(source_root.to_string_lossy(), Language::Java);
        self.invoke(&request, test_root_arguments(source_root, test_roots, targets))
    }

    fn invoke(&self, request: &CallGraphRequest, arguments: Vec<String>) -> CallGraphOutcome {
        let mut command = Command::new("java");
        command
            .arg("-jar")
            .arg(&self.jar)
            .args(["--algorithm", &self.algorithm])
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        isolate_process_group(&mut command);
        let child = command.spawn();
        let output = child.and_then(|child| {
            wait_for_output(child, self.timeout, &self.provider_id).map_err(std::io::Error::other)
        });
        match output {
            Ok(output) if output.status.success() => {
                parse_adapter_output(request, &output.stdout, &self.provider_id, &self.algorithm)
            }
            Ok(output) => failed(
                request,
                &self.provider_id,
                &self.algorithm,
                &String::from_utf8_lossy(&output.stderr),
            ),
            Err(error) => failed(request, &self.provider_id, &self.algorithm, &error.to_string()),
        }
    }
}

impl CallGraphProvider for JavaAdapterProvider {
    fn language(&self) -> Language {
        Language::Java
    }

    fn analyze(&self, request: &CallGraphRequest) -> CallGraphOutcome {
        self.invoke(
            request,
            vec!["--source-dir".to_owned(), request.source_root.clone()],
        )
    }
}

pub(crate) fn parse_adapter_output(
    request: &CallGraphRequest,
    output: &[u8],
    provider_id: &str,
    algorithm: &str,
) -> CallGraphOutcome {
    match serde_json::from_slice::<CallGraphOutcome>(output) {
        Ok(result) if result.schema_version == 2 => result,
        Ok(_) | Err(_) => failed(
            request,
            provider_id,
            algorithm,
            &format!("{provider_id} produced invalid call-graph JSON"),
        ),
    }
}

pub(crate) fn test_root_arguments(
    source_root: &std::path::Path,
    test_roots: &[JavaTestCase],
    targets: &[String],
) -> Vec<String> {
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
    for target in targets {
        arguments.push("--target".to_owned());
        arguments.push(target.clone());
    }
    arguments
}

#[cfg(unix)]
pub(crate) fn wait_for_output(
    mut child: Child,
    timeout: Duration,
    provider_id: &str,
) -> Result<Output, String> {
    let process_id = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_collector = std::thread::spawn(move || collect_stdout(stdout));
    let provider_id = provider_id.to_owned();
    let stderr_provider_id = provider_id.clone();
    let stderr_collector =
        std::thread::spawn(move || collect_stderr(stderr, &stderr_provider_id));
    let (sender, receiver) = mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let output: Result<Output, String> = (|| {
            let status = child.wait().map_err(|error| error.to_string())?;
            let stdout = stdout_collector
                .join()
                .map_err(|_| "adapter standard-output collector stopped unexpectedly".to_owned())??;
            let stderr = stderr_collector
                .join()
                .map_err(|_| "adapter standard-error collector stopped unexpectedly".to_owned())??;
            Ok(Output { status, stdout, stderr })
        })();
        let _ = sender.send(output);
    });
    let (output, timed_out) = match receiver.recv_timeout(timeout) {
        Ok(output) => (output, false),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            terminate_process_group(process_id);
            (
                receiver
                    .recv()
                    .map_err(|_| "adapter process-output collector stopped unexpectedly".to_owned())?,
                true,
            )
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err("adapter process-output collector stopped unexpectedly".into());
        }
    };
    if collector.join().is_err() {
        return Err("adapter process-output collector stopped unexpectedly".into());
    }
    let output = output?;
    if timed_out {
        return Err(timeout_diagnostic(timeout, &output.stderr, &provider_id));
    }
    Ok(output)
}

#[cfg(unix)]
fn collect_stdout(stdout: Option<ChildStdout>) -> Result<Vec<u8>, String> {
    let mut output = vec![];
    if let Some(mut stdout) = stdout {
        stdout.read_to_end(&mut output).map_err(|error| error.to_string())?;
    }
    Ok(output)
}

#[cfg(unix)]
fn collect_stderr(stderr: Option<ChildStderr>, provider_id: &str) -> Result<Vec<u8>, String> {
    let mut output = vec![];
    let Some(stderr) = stderr else {
        return Ok(output);
    };
    let mut lines = BufReader::new(stderr).split(b'\n');
    while let Some(line) = lines.next() {
        let line = line.map_err(|error| error.to_string())?;
        if line.windows(b"_phase=".len()).any(|window| window == b"_phase=") {
            eprintln!("{provider_id}: {}", String::from_utf8_lossy(&line));
        }
        output.extend_from_slice(&line);
        output.push(b'\n');
    }
    Ok(output)
}

#[cfg(unix)]
fn timeout_diagnostic(timeout: Duration, stderr: &[u8], provider_id: &str) -> String {
    const MAX_STDERR_BYTES: usize = 4_096;
    let tail_start = stderr.len().saturating_sub(MAX_STDERR_BYTES);
    let tail = String::from_utf8_lossy(&stderr[tail_start..]).trim().to_owned();
    if tail.is_empty() {
        format!("{provider_id} analysis timed out after {timeout:?}")
    } else {
        format!("{provider_id} analysis timed out after {timeout:?}; adapter stderr: {tail}")
    }
}

#[cfg(not(unix))]
pub(crate) fn wait_for_output(
    mut child: Child,
    timeout: Duration,
    provider_id: &str,
) -> Result<Output, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|error| error.to_string()),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{provider_id} analysis timed out after {timeout:?}"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

#[cfg(unix)]
pub(crate) fn isolate_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
pub(crate) fn isolate_process_group(_: &mut Command) {}

#[cfg(unix)]
fn terminate_process_group(process_id: u32) {
    let process_group = -(process_id as i32);
    let killed_group = unsafe { libc::kill(process_group, libc::SIGKILL) } == 0;
    if !killed_group {
        unsafe { libc::kill(process_id as i32, libc::SIGKILL) };
    }
}

fn failed(
    request: &CallGraphRequest,
    provider_id: &str,
    algorithm: &str,
    diagnostic: &str,
) -> CallGraphOutcome {
    CallGraphOutcome {
        schema_version: 1,
        status: CallGraphStatus::Failed,
        provider_id: provider_id.into(),
        source_root: request.source_root.clone(),
        algorithm: algorithm.into(),
        diagnostics: vec![diagnostic.into()],
        reachability: vec![],
        edges: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::{isolate_process_group, test_root_arguments, wait_for_output};
    use models::JavaTestCase;
    use std::{path::Path, process::{Command, Stdio}, time::Duration};

    #[test]
    fn preserves_phase_telemetry_when_an_adapter_times_out() {
        let child = Command::new("sh")
            .args(["-c", "echo callgraph_phase=building elapsed_ms=1 >&2; sleep 1"])
            .stderr(Stdio::piped())
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(10), "sootup-java")
            .expect_err("a process that exceeds its deadline must time out");

        assert!(error.contains("sootup-java analysis timed out"), "{error}");
        assert!(error.contains("callgraph_phase=building"), "{error}");
    }

    #[test]
    fn isolates_adapter_process_groups() {
        let mut command = Command::new("sh");
        isolate_process_group(&mut command);
        let child = command
            .args(["-c", "sleep 1"])
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(10), "sootup-java")
            .expect_err("a process that exceeds its deadline must time out");

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
            test_root_arguments(
                Path::new("module/src/main/java"),
                &tests,
                &["Lexample/Service#run/0".into()],
            ),
            vec![
                "--source-dir",
                "module/src/main/java",
                "--entrypoint",
                "Lexample/ServiceTest#changesService()V",
                "--target",
                "Lexample/Service#run/0",
            ]
        );
    }
}
