//! Writes the stage files under `match/` and their copies under `final/`.

use std::fs;
use std::path::Path;

use agentic_matcher::Metrics;
use agentic_models::Matches;
use anyhow::{Context, Result};
use serde::Serialize;

use crate::legacy::legacy;

pub fn write_all(out: &Path, matches: &Matches, metrics: &Metrics) -> Result<()> {
    let stage = out.join("match");
    let fin = out.join("final");
    let legacy = legacy(&matches.sdg);
    for dir in [&stage, &fin] {
        write(dir, "sdg.json", &matches.sdg)?;
        write(dir, "sdg.legacy.json", &legacy)?;
    }
    write(&stage, "unmatched.json", &matches.unmatched)?;
    write(&stage, "excluded.json", &matches.excluded)?;
    write(&stage, "metrics.json", metrics)
}

fn write<T: Serialize>(dir: &Path, name: &str, data: &T) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(name);
    let json = serde_json::to_string_pretty(data)?;
    fs::write(&path, json).with_context(|| format!("writing {}", path.display()))
}
