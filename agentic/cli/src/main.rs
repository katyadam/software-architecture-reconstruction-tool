//! Agentic SDG reconstruction: static matching, then Jev with `--jev`.

mod extract;
mod jev;
mod legacy;
mod output;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use agentic_matcher::jev::JevResult;
use anyhow::{Context, Result};
use clap::Parser;
use clients::typesafe::TypeSafeClient;
use models::ConfigurationData;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(about = "Agentic SDG reconstruction")]
struct Cli {
    /// Project to analyse.
    #[arg(short = 'p', long)]
    project_dir: PathBuf,
    /// Configuration JSON with service descriptions.
    #[arg(short = 'c', long)]
    config_file: PathBuf,
    /// Constants JSON (`{"constants": [{"name", "value"}]}`).
    #[arg(short = 'f', long)]
    constants_file: Option<PathBuf>,
    /// Output directory; `match/` and `final/` go below it.
    #[arg(short = 'o', long)]
    output_dir: PathBuf,
    /// Let Jev decide the calls static rules leave open; needs `TYPESAFE_API_KEY`.
    #[arg(long)]
    jev: bool,
}

#[derive(Deserialize)]
struct Constant {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct Constants {
    constants: Vec<Constant>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let client = match args.jev {
        true => Some(TypeSafeClient::new().context("--jev needs TYPESAFE_API_KEY")?),
        false => None,
    };
    anyhow::ensure!(
        args.project_dir.is_dir(),
        "project directory does not exist"
    );
    let config: ConfigurationData = read_json(&args.config_file)?;
    let constants: Vec<(String, String)> = match &args.constants_file {
        Some(path) => read_json::<Constants>(path)?
            .constants
            .into_iter()
            .map(|c| (c.name, c.value))
            .collect(),
        None => vec![],
    };
    let external: HashMap<String, String> = constants.iter().cloned().collect();

    let extracted = extract::extract(&args.project_dir, &external)?;
    let plan = agentic_matcher::plan(&extracted.ir, &config, &constants, args.jev)?;
    let r = match &client {
        Some(c) => jev::run(c, &plan, &extracted, &config, &external, &args.output_dir).await?,
        None => JevResult::default(),
    };
    let (matches, mut metrics) = plan.finish(r.links, r.unmatched, r.excluded);
    metrics.jev = client.is_some().then_some(r.metrics);
    output::write_all(&args.output_dir, &matches, &metrics)?;
    println!("{}", serde_json::to_string(&metrics)?);
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}
