use anyhow::{Context, Result};
use clap::Parser;
use extractor_runtime::dispatch;
use models::{CodeElementsAggregate, ConfigurationData};
use serde::Deserialize;
use std::path::PathBuf;
use std::{fs, time::Instant};
use synthesizer::{
    connectors::dto::Constant, direct_cm_build, direct_imcg_build, direct_sdg_build,
};
use uuid::Uuid;

mod crawler;

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

    let extraction = Instant::now();
    let all_code_elements = get_all_code_elements(&args.project_dir).await?;
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

async fn get_all_code_elements(project_dir: &PathBuf) -> Result<CodeElementsAggregate> {
    let mut aggregate = CodeElementsAggregate::default();
    let mut files_processed = 0;

    let paths = collect_files(project_dir)?;
    let files_to_process = paths.len();
    println!("Total files found: {}", files_to_process);

    for path in paths {
        files_processed += 1;
        let code = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "⚠️  Skipping file {:?}: {}",
                    path.file_name().unwrap_or_default(),
                    e
                );
                continue;
            }
        };
        println!(
            "Extracting ({files_processed}/{files_to_process}): {:?}",
            path.file_name().unwrap_or_default()
        );
        if let Some(file_elements) = dispatch::dispatch(&code, path.to_str().unwrap_or_default())
            .await
            .with_context(|| format!("Error dispatching file: {:?}", path))?
        {
            merge_aggregates(&mut aggregate, file_elements);
        }
    }

    Ok(aggregate)
}

fn collect_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_files(&path)?);
            } else {
                results.push(path);
            }
        }
    }
    Ok(results)
}

fn merge_aggregates(main: &mut CodeElementsAggregate, new: CodeElementsAggregate) {
    main.imports.extend(new.imports);
    main.entities.extend(new.entities);
    main.endpoints.extend(new.endpoints);
    main.restcalls.extend(new.restcalls);
    main.callables.extend(new.callables);
    main.call_statements.extend(new.call_statements);
}

fn save_json<T: serde::Serialize>(dir: &PathBuf, filename: &str, data: &T) -> Result<()> {
    let path = dir.join(filename);

    let json_data = serde_json::to_string_pretty(data)
        .with_context(|| format!("Failed to serialize {}", filename))?;

    fs::write(&path, json_data).with_context(|| format!("Failed to write file: {:?}", path))?;

    println!("   📄 Generated: {}", filename);
    Ok(())
}
