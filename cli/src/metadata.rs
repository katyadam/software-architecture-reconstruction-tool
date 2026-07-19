//! Run metadata emitted as `run_metadata.json` after each SAR run.

use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use synthesizer::contextmap::model::ContextMap;
use synthesizer::imcg::model::Imcg;
use synthesizer::sdg::model::Sdg;

/// Top-level run metadata emitted as `run_metadata.json` after each SAR run.
#[derive(Serialize)]
struct RunMetadata {
    run_number: u64,
    timestamp: String,
    benchmark: BenchmarkMeta,
    analyzer: AnalyzerMeta,
    config_file: String,
    constants_file: Option<String>,
    scrape_env: bool,
    llm: LlmMeta,
    timing: TimingMeta,
    results: ResultsMeta,
}

/// Identity of the analyzed benchmark project.
#[derive(Serialize)]
struct BenchmarkMeta {
    name: String,
    commit_hash: Option<String>,
    dirty: bool,
}

/// Identity of the analyzer (this CLI) itself.
#[derive(Serialize)]
struct AnalyzerMeta {
    version: String,
    commit_hash: Option<String>,
}

/// LLM arbiter configuration for the run.
#[derive(Serialize)]
struct LlmMeta {
    enabled: bool,
    model: String,
    url: String,
}

/// Wall-clock timing of the run, in milliseconds.
#[derive(Serialize)]
struct TimingMeta {
    extraction_ms: u64,
    synthesis_ms: u64,
    total_ms: u64,
}

/// Element counts for each synthesized architectural view.
#[derive(Serialize)]
struct ResultsMeta {
    context_map_entities: usize,
    context_map_dependencies: usize,
    sdg_services: usize,
    sdg_connections: usize,
    imcg_callables: usize,
    imcg_calls: usize,
}

/// Run `git -C <dir> <args...>` and return trimmed stdout, or `None` on any failure.
pub(crate) fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Compute the next per-benchmark run number by scanning sibling directories of the
/// output dir for `run_metadata.json` files whose `benchmark.name` matches. Returns
/// `count_of_matching_previous_runs + 1`. Never fails: unreadable/unparseable entries
/// and the current output dir itself are silently skipped.
pub fn next_run_number(parent: &Path, output_dir: &Path, benchmark: &str) -> u64 {
    let this = fs::canonicalize(output_dir).ok();
    let entries = match fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return 1,
    };

    let mut count = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip our own output dir (guards against re-running into an existing dir).
        if this
            .as_ref()
            .is_some_and(|a| fs::canonicalize(&path).is_ok_and(|b| *a == b))
        {
            continue;
        }
        let meta_path = path.join("run_metadata.json");
        let raw = match fs::read_to_string(&meta_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if json
            .get("benchmark")
            .and_then(|b| b.get("name"))
            .and_then(|n| n.as_str())
            == Some(benchmark)
        {
            count += 1;
        }
    }
    count + 1
}

/// Borrowed inputs required to build and emit `run_metadata.json`.
pub struct RunMetadataInput<'a> {
    pub output_dir: &'a Path,
    pub project_dir: &'a Path,
    pub config_file: &'a Path,
    pub constants_file: Option<&'a Path>,
    pub scrape_env: bool,
    pub llm_enabled: bool,
    pub llm_model: &'a str,
    pub llm_url: &'a str,
    pub cm: &'a ContextMap,
    pub sdg: &'a Sdg,
    pub imcg: &'a Imcg,
    pub extraction_elapsed: Duration,
    pub synthesis_elapsed: Duration,
}

/// Build `RunMetadata` from the run's inputs and write it to `run_metadata.json`.
pub fn write_run_metadata(input: RunMetadataInput<'_>) -> Result<()> {
    let benchmark_name = fs::canonicalize(input.project_dir)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "unknown".to_string());

    let parent = input
        .output_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let run_number = next_run_number(&parent, input.output_dir, &benchmark_name);

    let extraction_ms = input.extraction_elapsed.as_millis() as u64;
    let synthesis_ms = input.synthesis_elapsed.as_millis() as u64;

    let metadata = RunMetadata {
        run_number,
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        benchmark: BenchmarkMeta {
            name: benchmark_name,
            commit_hash: git_output(input.project_dir, &["rev-parse", "HEAD"]),
            dirty: git_output(input.project_dir, &["status", "--porcelain"]).is_some(),
        },
        analyzer: AnalyzerMeta {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit_hash: git_output(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &["rev-parse", "--short", "HEAD"],
            ),
        },
        config_file: input.config_file.to_string_lossy().into_owned(),
        constants_file: input
            .constants_file
            .map(|p| p.to_string_lossy().into_owned()),
        scrape_env: input.scrape_env,
        llm: LlmMeta {
            enabled: input.llm_enabled,
            model: input.llm_model.to_string(),
            url: input.llm_url.to_string(),
        },
        timing: TimingMeta {
            extraction_ms,
            synthesis_ms,
            total_ms: extraction_ms + synthesis_ms,
        },
        results: ResultsMeta {
            context_map_entities: input.cm.entities.len(),
            context_map_dependencies: input.cm.dependencies.len(),
            sdg_services: input.sdg.services.len(),
            sdg_connections: input.sdg.connections.len(),
            imcg_callables: input.imcg.callables.len(),
            imcg_calls: input.imcg.calls.len(),
        },
    };
    crate::save_json(input.output_dir, "run_metadata.json", &metadata)
}
