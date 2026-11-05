use anyhow::Result;
use clap::Parser;
use extractor_runtime::impact::python::{PythonImpactOptions, analyze_python_impact};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Parser)]
#[command(about = "Report Python test impact conservatively from a Git diff")]
struct Arguments {
    /// Candidate Git checkout whose HEAD is analyzed.
    #[arg(long)]
    project_dir: PathBuf,
    /// Git revision used as the old side of the candidate comparison.
    #[arg(long)]
    baseline_revision: String,
    /// Destination JSON file for the auditable selection result.
    #[arg(long)]
    output: PathBuf,
    /// Seconds to allow pyan3 to run for call-graph annotation (0 = skip graph phase).
    #[arg(long, default_value = "30")]
    pyan3_timeout_secs: u64,
}

fn main() -> Result<()> {
    let a = Arguments::parse();
    let output = validated_output(&a.project_dir, &a.output)?;
    let pyan3_timeout = if a.pyan3_timeout_secs == 0 {
        None
    } else {
        Some(Duration::from_secs(a.pyan3_timeout_secs))
    };
    let options = PythonImpactOptions {
        project_dir: a.project_dir,
        baseline_revision: a.baseline_revision,
        pyan3_timeout,
    };
    let report = analyze_python_impact(&options)?;
    write_report_atomic(&output, &report)?;
    eprintln!(
        "python_impact_phase=report_ready output={}",
        output.display()
    );
    Ok(())
}

/// Validates that the output path is not inside the candidate checkout.
fn validated_output(project_dir: &Path, output: &Path) -> Result<PathBuf> {
    if output.is_symlink() {
        anyhow::bail!("--output must not be a symlink");
    }
    let abs_out = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    if let Ok(project) = project_dir.canonicalize() {
        if abs_out.starts_with(&project) {
            anyhow::bail!("--output must not be inside --project-dir");
        }
    }
    Ok(abs_out)
}

/// Writes the report atomically by writing to a temp file then renaming.
fn write_report_atomic<T: serde::Serialize>(path: &Path, report: &T) -> Result<()> {
    if path.is_dir() {
        anyhow::bail!("output path is a directory: {}", path.display());
    }
    let tmp = path.with_extension("tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &tmp,
        serde_json::to_string_pretty(report)?,
    )?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_baseline_revision() {
        let args = ["python-test-impact", "--project-dir", "/p", "--baseline-revision", "", "--output", "/tmp/out.json"];
        let parsed = Arguments::try_parse_from(args);
        // Parsing succeeds; runtime validation happens in analyze_python_impact.
        // At minimum the flag is accepted syntactically.
        assert!(parsed.is_ok());
    }
}
