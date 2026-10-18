use clap::Parser;
use cli::go_impact_output::{validate_output_path, write_report_atomic};
use extractor_runtime::impact::go::{GoImpactOptions, analyze_go_impact};
use std::{path::PathBuf, time::Duration};
#[derive(Parser)]
#[command(about = "Report Go graph suggestions separately from conservative test execution")]
struct Arguments {
    #[arg(long)]
    project_dir: PathBuf,
    #[arg(long)]
    baseline_revision: String,
    #[arg(long)]
    go_callgraph_binary: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "")]
    build_tags: String,
    #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(1..))]
    analysis_timeout_seconds: u64,
}
fn main() -> anyhow::Result<()> {
    let a = Arguments::parse();
    let output = validate_output_path(&a.project_dir, &a.output)?;
    let result = analyze_go_impact(&GoImpactOptions {
        project_dir: a.project_dir,
        baseline_revision: a.baseline_revision,
        go_callgraph_binary: a.go_callgraph_binary,
        build_tags: a.build_tags,
        analysis_timeout: Duration::from_secs(a.analysis_timeout_seconds),
    })?;
    write_report_atomic(&output, &result)?;
    eprintln!("go_impact_phase=report_ready output={}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_defaults_to_300_and_rejects_zero() {
        let args = [
            "go-test-impact",
            "--project-dir",
            "/p",
            "--baseline-revision",
            "HEAD",
            "--go-callgraph-binary",
            "/bin/native",
            "--output",
            "/tmp/report.json",
        ];
        assert_eq!(
            Arguments::try_parse_from(args)
                .unwrap()
                .analysis_timeout_seconds,
            300
        );
        assert!(
            Arguments::try_parse_from(args.into_iter().chain(["--analysis-timeout-seconds", "0"]))
                .is_err()
        );
    }
}
