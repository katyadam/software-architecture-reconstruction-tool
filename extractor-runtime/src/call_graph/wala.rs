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

const DEFAULT_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(300);

/// Invokes the standalone WALA adapter for Java source trees.
pub struct WalaJavaProvider {
    jar: PathBuf,
    timeout: Duration,
    algorithm: String,
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
            algorithm: std::env::var("WALA_ALGORITHM").unwrap_or_else(|_| "cha".into()),
        }
    }

    /// Analyzes a Maven module from discovered JUnit methods instead of only Java main methods.
    pub fn analyze_test_roots(
        &self,
        source_root: &std::path::Path,
        test_roots: &[JavaTestCase],
        targets: &[String],
    ) -> CallGraphOutcome {
        let request = CallGraphRequest::new(source_root.to_string_lossy(), Language::Java);
        self.invoke(
            &request,
            test_root_arguments(source_root, test_roots, targets),
        )
    }

    /// Launches the adapter with the supplied fully formed argument vector.
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
        let output = child
            .and_then(|child| wait_for_output(child, self.timeout).map_err(std::io::Error::other));
        match output {
            Ok(output) if output.status.success() => parse_adapter_output(request, &output.stdout),
            Ok(output) => failed(request, &String::from_utf8_lossy(&output.stderr)),
            Err(error) => failed(request, &error.to_string()),
        }
    }
}

/// Accepts versioned adapter outcomes, including structured failures with actionable diagnostics.
fn parse_adapter_output(request: &CallGraphRequest, output: &[u8]) -> CallGraphOutcome {
    match serde_json::from_slice::<CallGraphOutcome>(output) {
        Ok(result) if result.schema_version == 2 => result,
        Ok(_) | Err(_) => failed(request, "WALA produced invalid call-graph JSON"),
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
fn test_root_arguments(
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

/// Waits for an adapter process and terminates it when it exceeds the configured deadline.
#[cfg(unix)]
fn wait_for_output(mut child: Child, timeout: Duration) -> Result<Output, String> {
    let process_id = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_collector = std::thread::spawn(move || collect_stdout(stdout));
    let stderr_collector = std::thread::spawn(move || collect_stderr(stderr));
    let (sender, receiver) = mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let output: Result<Output, String> = (|| {
            let status = child.wait().map_err(|error| error.to_string())?;
            let stdout = stdout_collector
                .join()
                .map_err(|_| "WALA standard-output collector stopped unexpectedly".to_owned())??;
            let stderr = stderr_collector
                .join()
                .map_err(|_| "WALA standard-error collector stopped unexpectedly".to_owned())??;
            Ok(Output {
                status,
                stdout,
                stderr,
            })
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
                    .map_err(|_| "WALA process-output collector stopped unexpectedly".to_owned())?,
                true,
            )
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err("WALA process-output collector stopped unexpectedly".into());
        }
    };
    if collector.join().is_err() {
        return Err("WALA process-output collector stopped unexpectedly".into());
    }
    let output = output?;

    if timed_out {
        return Err(timeout_diagnostic(timeout, &output.stderr));
    }
    Ok(output)
}

/// Collects adapter JSON without exposing it before the adapter exits successfully.
#[cfg(unix)]
fn collect_stdout(stdout: Option<ChildStdout>) -> Result<Vec<u8>, String> {
    let mut output = vec![];
    if let Some(mut stdout) = stdout {
        stdout
            .read_to_end(&mut output)
            .map_err(|error| error.to_string())?;
    }
    Ok(output)
}

/// Relays structured adapter phase records immediately and retains all stderr for diagnostics.
#[cfg(unix)]
fn collect_stderr(stderr: Option<ChildStderr>) -> Result<Vec<u8>, String> {
    let mut output = vec![];
    let Some(stderr) = stderr else {
        return Ok(output);
    };
    let mut lines = BufReader::new(stderr).split(b'\n');
    while let Some(line) = lines.next() {
        let line = line.map_err(|error| error.to_string())?;
        if line.starts_with(b"wala_phase=") {
            eprintln!("WALA: {}", String::from_utf8_lossy(&line));
        }
        output.extend_from_slice(&line);
        output.push(b'\n');
    }
    Ok(output)
}

/// Makes a timeout actionable without allowing unbounded adapter stderr into result JSON.
#[cfg(unix)]
fn timeout_diagnostic(timeout: Duration, stderr: &[u8]) -> String {
    const MAX_STDERR_BYTES: usize = 4_096;
    let tail_start = stderr.len().saturating_sub(MAX_STDERR_BYTES);
    let tail = String::from_utf8_lossy(&stderr[tail_start..])
        .trim()
        .to_owned();
    if tail.is_empty() {
        format!("WALA analysis timed out after {timeout:?}")
    } else {
        format!("WALA analysis timed out after {timeout:?}; adapter stderr: {tail}")
    }
}

#[cfg(not(unix))]
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

#[cfg(unix)]
fn isolate_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn isolate_process_group(_: &mut Command) {}

#[cfg(unix)]
fn terminate_process_group(process_id: u32) {
    let process_group = -(process_id as i32);
    let killed_group = unsafe { libc::kill(process_group, libc::SIGKILL) } == 0;
    if !killed_group {
        unsafe { libc::kill(process_id as i32, libc::SIGKILL) };
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
        reachability: vec![],
        edges: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        isolate_process_group, parse_adapter_output, test_root_arguments, wait_for_output,
    };
    use models::{
        JavaTestCase,
        call_graph::{CallGraphRequest, CallGraphStatus, Language},
    };
    use std::{
        path::Path,
        process::Command,
        thread,
        time::{Duration, Instant},
    };
    use tempfile::tempdir;

    #[test]
    fn stops_a_child_process_that_exceeds_the_deadline() {
        let child = Command::new("sleep")
            .arg("1")
            .spawn()
            .expect("test command should start");
        let child_pid = child.id().to_string();

        let error = wait_for_output(child, Duration::from_millis(10))
            .expect_err("a process that sleeps longer than its deadline must time out");

        assert!(error.contains("timed out"), "{error}");
        assert!(
            !process_exists(&child_pid),
            "direct adapter process {child_pid} should be reaped before timeout returns"
        );
    }

    #[test]
    fn preserves_adapter_phase_telemetry_when_a_process_times_out() {
        let child = Command::new("sh")
            .args([
                "-c",
                "echo wala_phase=call_graph_ready elapsed_ms=1 >&2; sleep 1",
            ])
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("test command should start");

        let error = wait_for_output(child, Duration::from_millis(100))
            .expect_err("a process that sleeps longer than its deadline must time out");

        assert!(error.contains("timed out"), "{error}");
        assert!(error.contains("wala_phase=call_graph_ready"), "{error}");
    }

    #[test]
    fn stops_descendants_when_a_process_group_exceeds_the_deadline() {
        let directory = tempdir().expect("temporary directory should exist");
        let pid_file = directory.path().join("descendant.pid");
        let script = format!("sleep 30 & echo $! > '{}' && wait", pid_file.display());
        let mut command = Command::new("sh");
        command.args(["-c", &script]);
        isolate_process_group(&mut command);
        let child = command.spawn().expect("test command should start");

        let deadline = Instant::now() + Duration::from_secs(1);
        while !pid_file.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let descendant_pid = std::fs::read_to_string(&pid_file)
            .expect("parent should record the descendant PID")
            .trim()
            .to_owned();

        let error = wait_for_output(child, Duration::from_millis(10))
            .expect_err("a process group that exceeds its deadline must time out");

        assert!(error.contains("timed out"), "{error}");
        assert!(
            !process_exists(&descendant_pid),
            "descendant process {descendant_pid} should be terminated with its parent"
        );
    }

    #[test]
    fn stops_descendants_that_hold_output_pipes_after_the_parent_exits() {
        let directory = tempdir().expect("temporary directory should exist");
        let pid_file = directory.path().join("descendant.pid");
        let script = format!("sleep 1 & echo $! > '{}'", pid_file.display());
        let mut command = Command::new("sh");
        command
            .args(["-c", &script])
            .stdout(std::process::Stdio::piped());
        isolate_process_group(&mut command);
        let child = command.spawn().expect("test command should start");

        let deadline = Instant::now() + Duration::from_secs(1);
        while !pid_file.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let descendant_pid = std::fs::read_to_string(&pid_file)
            .expect("parent should record the descendant PID")
            .trim()
            .to_owned();

        let started = Instant::now();
        let error = wait_for_output(child, Duration::from_millis(10))
            .expect_err("descendants retaining output pipes must still time out");

        assert!(error.contains("timed out"), "{error}");
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "timeout should remain effective while collecting output"
        );
        assert!(
            !process_exists(&descendant_pid),
            "descendant process {descendant_pid} should be terminated"
        );
    }

    fn process_exists(pid: &str) -> bool {
        Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
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
                &["Lexample/Service#run/0".into()]
            ),
            vec![
                "--source-dir".to_owned(),
                "module/src/main/java".to_owned(),
                "--entrypoint".to_owned(),
                "Lexample/ServiceTest#changesService()V".to_owned(),
                "--target".to_owned(),
                "Lexample/Service#run/0".to_owned(),
            ]
        );
    }

    #[test]
    fn parses_schema_v2_reachability_from_a_valid_adapter_outcome() {
        let request = CallGraphRequest::new("/service", Language::Java);
        let result = parse_adapter_output(
            &request,
            br#"{"schema_version":2,"status":"ok","provider_id":"wala-java","source_root":"/service","algorithm":"cha_bytecode","diagnostics":[],"reachability":[{"method":{"language":"java","declaring_type":"Lexample/Service","member_name":"changed","descriptor":"()V","source_path":null,"source_line":null},"entrypoints":[{"language":"java","declaring_type":"Lexample/ServiceTest","member_name":"coversChanged","descriptor":"()V","source_path":null,"source_line":null}]}],"edges":[]}"#,
        );

        assert_eq!(result.status, CallGraphStatus::Ok);
        assert_eq!(result.reachability.len(), 1,);
        assert_eq!(result.reachability[0].method.member_name, "changed");
    }

    #[test]
    fn rejects_malformed_and_unsupported_version_adapter_output() {
        let request = CallGraphRequest::new("/service", Language::Java);
        for output in [
            br#"not json"#.as_slice(),
            br#"{"schema_version":1,"status":"failed","provider_id":"wala-java","source_root":"/service","algorithm":"cha_bytecode","diagnostics":["legacy schema"],"edges":[]}"#
                .as_slice(),
        ] {
            let result = parse_adapter_output(&request, output);
            assert_eq!(result.status, CallGraphStatus::Failed);
            assert_eq!(result.diagnostics, vec!["WALA produced invalid call-graph JSON"]);
        }
    }
}
