use anyhow::{Context, Result};
use clap::Parser;
use cli::metadata::{self, RunMetadataInput};
use cli::{enrich_with_wala_inputs, get_all_code_elements, save_json};
use clients::typesafe::{SystemOneRequest, TypeSafeClient};
use extractor_runtime::pipeline::pass3::{DdfaMode, Resolver};
use models::ConfigurationData;
use sage::resolver::client::SageClient;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::{
    fs,
    time::{Duration, Instant},
};
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build_with_unmatched,
    tie_break,
};
use uuid::Uuid;

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ResolverArg {
    Matcher,
    Sage,
    Once,
    Acquire,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Local simulation of the extraction process")]
struct Cli {
    #[arg(short = 'p', long, value_name = "DIR")]
    project_dir: PathBuf,

    #[arg(short = 'c', long, value_name = "FILE")]
    config_file: PathBuf,

    #[arg(short = 'f', long, value_name = "FILE")]
    constants_file: Option<PathBuf>,

    #[arg(short = 'o', long, value_name = "DIR")]
    output_dir: PathBuf,

    #[arg(long, default_value_t = false)]
    scrape: bool,

    #[arg(long, default_value_t = false)]
    llm: bool,

    #[arg(long, default_value = "http://localhost:11434/v1")]
    llm_url: String,

    #[arg(long, default_value = "qwen2.5-coder:7b")]
    llm_model: String,

    /// Backend for residuals the matcher misses (with --llm). once/acquire need --typesafe.
    #[arg(long, value_enum, default_value_t = ResolverArg::Sage)]
    resolver: ResolverArg,

    #[arg(long, default_value_t = false)]
    typesafe: bool,

    /// Ask Jev to pick endpoints for ambiguous or unmatched REST calls
    #[arg(long, default_value_t = false)]
    jev_match: bool,
    /// Optional standalone WALA adapter used to enrich Java call edges.
    #[arg(long, value_name = "FILE")]
    wala_adapter_jar: Option<PathBuf>,
    /// Maximum time allowed for one WALA call-graph analysis.
    #[arg(long, default_value_t = 300, value_name = "SECONDS")]
    wala_timeout_seconds: u64,
}

#[derive(Deserialize)]
struct AnonymizedConstant {
    name: String,
    value: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ConstantsDto {
    commit_hash: String,
    constants: Vec<AnonymizedConstant>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();
    let args = Cli::parse();

    if matches!(args.resolver, ResolverArg::Once | ResolverArg::Acquire) && !args.typesafe {
        anyhow::bail!("--resolver once|acquire needs --typesafe");
    }
    if args.jev_match && !args.typesafe {
        anyhow::bail!("--jev-match needs --typesafe");
    }
    let jev_match_min: f64 = match std::env::var("JEV_MATCH_MIN") {
        Ok(v) if args.jev_match => v.parse().context("JEV_MATCH_MIN must be a number")?,
        _ => 0.2,
    };

    if !args.project_dir.is_dir() {
        anyhow::bail!("Project directory does not exist!");
    }

    let config_raw = fs::read_to_string(&args.config_file)
        .context("Failed to read configuration file from disk")?;

    let config: ConfigurationData = serde_json::from_str(&config_raw)
        .context("Failed to parse Configuration JSON - check your schema!")?;

    let java_inputs = if args.wala_adapter_jar.is_some()
        && config.service_descriptions.iter().any(|s| {
            s.java_analysis
                .as_ref()
                .is_some_and(|a| a.artifacts_file.is_some() || !a.maven_profiles.is_empty())
        }) {
        let output = std::process::Command::new("git")
            .current_dir(&args.project_dir)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .context("Unable to read Java manifest candidate revision")?;
        anyhow::ensure!(
            output.status.success(),
            "Java manifests require a candidate Git checkout: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let revision = String::from_utf8(output.stdout)?;
        Some(extractor_runtime::java_analysis::load_java_analysis_inputs(
            &args.project_dir,
            &args.project_dir,
            &config,
            revision.trim(),
        )?)
    } else {
        None
    };

    let constants_dto = if let Some(path) = &args.constants_file {
        let raw = fs::read_to_string(path).context("Failed to read constants file from disk")?;

        serde_json::from_str(&raw).context("Failed to parse Constants JSON")?
    } else {
        println!("ℹ️  No constants file provided, using empty defaults.");
        ConstantsDto {
            constants: vec![],
            commit_hash: "".to_string(),
        }
    };

    fs::create_dir_all(&args.output_dir)?;

    println!(
        "🔍 Starting Software Architecture Reconstruction in: {:?}",
        args.project_dir
    );

    // Build external constants map: all constants from the file, including dotted-path
    // keys like "settings.as_url" that are injected into the symbolic evaluator.
    let mut external_constants: HashMap<String, String> = constants_dto
        .constants
        .iter()
        .map(|c| (c.name.clone(), c.value.clone()))
        .collect();

    if args.scrape {
        println!("🔍 Scraping env files in {:?}...", args.project_dir);
        let scraped = env_scraper::scrape(&args.project_dir);
        let count = scraped.len();
        for (k, v) in scraped {
            external_constants.entry(k).or_insert(v);
        }
        println!("   Found {count} env vars from .env / docker-compose files.");
    }

    let sage = (args.llm && args.resolver == ResolverArg::Sage)
        .then(|| SageClient::new(&args.llm_url, &args.llm_model));
    let resolver = args.llm.then_some(match (args.resolver, &sage) {
        (ResolverArg::Sage, Some(sage)) => Resolver::Sage(sage),
        (ResolverArg::Once, _) => Resolver::Ddfa(DdfaMode::Once),
        (ResolverArg::Acquire, _) => Resolver::Ddfa(DdfaMode::Acquire),
        _ => Resolver::Matcher,
    });

    let typesafe_client = args
        .typesafe
        .then(|| TypeSafeClient::new().expect("TypeSafe client to instantiate"));

    let extraction = Instant::now();
    let all_code_elements = enrich_with_wala_inputs(
        get_all_code_elements(
            &args.project_dir,
            &external_constants,
            &config,
            resolver,
            typesafe_client.as_ref(),
        )
        .await?,
        &args.project_dir,
        &config,
        args.wala_adapter_jar.as_deref(),
        Duration::from_secs(args.wala_timeout_seconds),
        java_inputs.as_ref(),
    );
    let extraction_elapsed = extraction.elapsed();

    println!("✅ Extraction successful!");
    println!("⚙️ Starting Synthesis process..");

    let synthesis = Instant::now();
    let cm = direct_cm_build(&all_code_elements, &config);
    let deanonymized_constants: Vec<Constant> = constants_dto
        .constants
        .into_iter()
        .map(|c| Constant::new(Uuid::new_v4(), c.name, c.value))
        .collect();

    let (mut sdg, unmatched) =
        direct_sdg_build_with_unmatched(&all_code_elements, &config, &deanonymized_constants);
    if let (true, Some(client)) = (args.jev_match, typesafe_client.as_ref()) {
        let ask = async |req: &SystemOneRequest| client.system_one(req).await;
        let stats = tie_break(&mut sdg, &unmatched, jev_match_min, ask).await;
        println!(
            "[match-jev] total calls={} input_tokens={}",
            stats.calls, stats.input_tokens
        );
    }
    let imcg = direct_imcg_build(&all_code_elements, &config, &sdg);

    let synthesis_elapsed = synthesis.elapsed();

    println!("✅ Synthesis successful! Saving results...");

    save_json(&args.output_dir, "context_map.json", &cm)?;
    save_json(&args.output_dir, "sdg.json", &sdg)?;
    save_json(&args.output_dir, "imcg.json", &imcg)?;

    // Emit run_metadata.json alongside the graphs.
    metadata::write_run_metadata(RunMetadataInput {
        output_dir: &args.output_dir,
        project_dir: &args.project_dir,
        config_file: &args.config_file,
        constants_file: args.constants_file.as_deref(),
        scrape_env: args.scrape,
        llm_enabled: args.llm,
        llm_model: &args.llm_model,
        llm_url: &args.llm_url,
        resolver: &format!("{:?}", args.resolver).to_lowercase(),
        cm: &cm,
        sdg: &sdg,
        imcg: &imcg,
        extraction_elapsed,
        synthesis_elapsed,
    })?;

    println!("✅ SAR complete! Results saved to: {:?}", args.output_dir);
    println!(
        "⏳ Total time:\n\tExtraction: {:?}\n\tSynthesis: {:?}",
        extraction_elapsed, synthesis_elapsed
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn accepts_a_wala_analysis_timeout() {
        let cli = Cli::try_parse_from([
            "voyantclair",
            "--project-dir",
            "project",
            "--config-file",
            "config.json",
            "--output-dir",
            "output",
            "--wala-timeout-seconds",
            "12",
        ])
        .expect("the CLI should accept a WALA timeout");

        assert_eq!(cli.wala_timeout_seconds, 12);
    }
}
