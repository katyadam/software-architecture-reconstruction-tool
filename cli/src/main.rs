use anyhow::{Context, Result};
use clap::Parser;
use cli::get_all_code_elements;
use models::ConfigurationData;
use sage::resolver::client::SageClient;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, time::Instant};
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build,
};
use uuid::Uuid;

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
    let args = Cli::parse();

    if !args.project_dir.is_dir() {
        anyhow::bail!("Project directory does not exist!");
    }

    let config_raw = fs::read_to_string(&args.config_file)
        .context("Failed to read configuration file from disk")?;

    let config: ConfigurationData = serde_json::from_str(&config_raw)
        .context("Failed to parse Configuration JSON - check your schema!")?;

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

    let sage = args
        .llm
        .then(|| SageClient::new(&args.llm_url, &args.llm_model, 0.7, 150));

    let extraction = Instant::now();
    let all_code_elements = get_all_code_elements(
        &args.project_dir,
        &external_constants,
        &config,
        sage.as_ref(),
    )
    .await?;
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

    let sdg = direct_sdg_build(&all_code_elements, &config, &deanonymized_constants);
    let imcg = direct_imcg_build(&all_code_elements, &config, &sdg);

    let synthesis_elapsed = synthesis.elapsed();

    println!("✅ Synthesis successful! Saving results...");

    save_json(&args.output_dir, "context_map.json", &cm)?;
    save_json(&args.output_dir, "sdg.json", &sdg)?;
    save_json(&args.output_dir, "imcg.json", &imcg)?;

    println!("✅ SAR complete! Results saved to: {:?}", args.output_dir);
    println!(
        "⏳ Total time:\n\tExtraction: {:?}\n\tSynthesis: {:?}",
        extraction_elapsed, synthesis_elapsed
    );
    Ok(())
}

fn save_json<T: serde::Serialize>(dir: &Path, filename: &str, data: &T) -> Result<()> {
    let path = dir.join(filename);

    let json_data = serde_json::to_string_pretty(data)
        .with_context(|| format!("Failed to serialize {}", filename))?;

    fs::write(&path, json_data).with_context(|| format!("Failed to write file: {:?}", path))?;

    println!("   📄 Generated: {}", filename);
    Ok(())
}
