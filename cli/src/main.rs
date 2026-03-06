use anyhow::{Context, Result};
use clap::Parser;
use extractor_runtime::dispatch;
use models::{CodeElementsAggregate, ConfigurationData};
use std::fs;
use std::path::PathBuf;
use synthesizer::connectors::dto::ConstantsDto;
use synthesizer::{direct_cm_build, direct_imcg_build, direct_sdg_build};

mod crawler;

#[derive(Parser, Debug)]
#[command(author, version, about = "Local simulation of the extraction process")]
struct Cli {
    #[arg(short, long, value_name = "DIR")]
    project_dir: PathBuf,

    #[arg(short, long, value_name = "FILE")]
    config_file: PathBuf,

    #[arg(short, long, value_name = "FILE")]
    constants_file: PathBuf,

    #[arg(short, long, value_name = "DIR")]
    output_dir: PathBuf,
}

// 1. Added the tokio runtime attribute
#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    println!("🚀 Starting Software Architecture Reconstruction...");
    println!("📂 Project: {:?}", args.project_dir);

    if !args.project_dir.is_dir() {
        anyhow::bail!("Project directory does not exist!");
    }

    let config_raw = fs::read_to_string(&args.config_file)
        .context("Failed to read configuration file from disk")?;

    let config: ConfigurationData = serde_json::from_str(&config_raw)
        .context("Failed to parse Configuration JSON - check your schema!")?;

    // 2. Read and Parse Constants
    let constants_raw = fs::read_to_string(&args.constants_file)
        .context("Failed to read constants file from disk")?;

    let constantsDto: ConstantsDto = serde_json::from_str(&constants_raw)
        .context("Failed to parse Constants JSON - check your schema!")?;

    fs::create_dir_all(&args.output_dir)?;

    let all_code_elements = get_all_code_elements(&args.project_dir, &args.output_dir).await?;

    let cm = direct_cm_build(&all_code_elements, &config);
    let sdg = direct_sdg_build(&all_code_elements, &config, &constantsDto.constants);
    let imcg = direct_imcg_build(&all_code_elements, &config, &sdg);

    println!("✅ SAR complete. Results saved to: {:?}", args.output_dir);
    Ok(())
}

async fn get_all_code_elements(input: &PathBuf, output: &PathBuf) -> Result<CodeElementsAggregate> {
    println!("--- Processing files in {:?} ---", input);

    let code_elements = dispatch::dispatch("code", "filepath").await?;

    Ok(CodeElementsAggregate {
        imports: vec![],
        entities: vec![],
        endpoints: vec![],
        restcalls: vec![],
        callables: vec![],
        call_statements: vec![],
    })
}
