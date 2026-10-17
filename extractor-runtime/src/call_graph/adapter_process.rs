//! Language-neutral process transport. No build preparation or graph semantics
//! belong here; Java JARs and the Go executable remain independent providers.

use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, ChildStderr, ChildStdout, Command, Output},
    sync::mpsc,
    time::Duration,
};
#[cfg(not(unix))]
use std::{thread, time::Instant};

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
    let stderr_collector = std::thread::spawn(move || collect_stderr(stderr, &stderr_provider_id));
    let (sender, receiver) = mpsc::sync_channel(1);
    let collector = std::thread::spawn(move || {
        let output: Result<Output, String> = (|| {
            let status = child.wait().map_err(|error| error.to_string())?;
            let stdout = stdout_collector.join().map_err(|_| {
                "adapter standard-output collector stopped unexpectedly".to_owned()
            })??;
            let stderr = stderr_collector.join().map_err(|_| {
                "adapter standard-error collector stopped unexpectedly".to_owned()
            })??;
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
                receiver.recv().map_err(|_| {
                    "adapter process-output collector stopped unexpectedly".to_owned()
                })?,
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
        stdout
            .read_to_end(&mut output)
            .map_err(|error| error.to_string())?;
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
        if line
            .windows(b"_phase=".len())
            .any(|window| window == b"_phase=")
        {
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
    let tail = String::from_utf8_lossy(&stderr[tail_start..])
        .trim()
        .to_owned();
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
                return Err(format!(
                    "{provider_id} analysis timed out after {timeout:?}"
                ));
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
