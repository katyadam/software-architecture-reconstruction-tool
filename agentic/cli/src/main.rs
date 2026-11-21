//! Agentic SDG reconstruction. This slice: static matching only, no model.

mod extract;
mod legacy;
mod output;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
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

fn main() -> Result<()> {
    let args = Cli::parse();
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

    let ir = extract::extract(&args.project_dir, &external)?;
    let (matches, metrics) = agentic_matcher::match_static(&ir, &config, &constants)?;
    output::write_all(&args.output_dir, &matches, &metrics)?;
    println!("{}", serde_json::to_string(&metrics)?);
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}
