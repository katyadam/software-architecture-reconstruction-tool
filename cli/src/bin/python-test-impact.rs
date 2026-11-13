use anyhow::Result;
use clap::Parser;
use cli::go_impact_output::{validate_output_path, write_report_atomic};
use extractor_runtime::impact::python::{PythonImpactOptions, analyze_python_impact};
use std::{path::PathBuf, time::Duration};

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
    let output = validate_output_path(&a.project_dir, &a.output)?;
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
