//! Parser abstractions for `.env` and docker-compose environment variable files.

use std::path::Path;

pub mod docker_compose;
pub mod dotenv;

use crate::discovery::{DiscoveredFile, dotenv_priority};
use docker_compose::DockerComposeParser;
use dotenv::DotEnvParser;

/// Classifies the kind of file a parser handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    DotEnv,
    DockerCompose,
}

impl SourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            SourceKind::DotEnv => "dotenv",
            SourceKind::DockerCompose => "docker_compose",
        }
    }
}

/// A single environment-variable entry extracted from a file.
#[derive(Debug, Clone)]
pub struct EnvEntry {
    pub name: String,
    pub value: String,
    /// Absolute path to the file the entry came from.
    pub source_path: String,
    /// File kind that produced this entry.
    pub kind: SourceKind,
    /// Merge priority (higher wins). See the plan's conflict-resolution table.
    pub priority: u8,
}

/// Implemented by each file-format parser.
pub trait Parser {
    fn parse(&self, content: &str, path: &Path) -> Vec<EnvEntry>;
}

/// Each file is read from disk; unreadable files are skipped with a warning.
pub fn parse_all(files: &[DiscoveredFile]) -> Vec<EnvEntry> {
    use log::warn;
    use std::fs;

    let mut entries = Vec::new();

    for file in files {
        let content = match fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(e) => {
                warn!("parse_all: cannot read {:?}: {}", file.path, e);
                continue;
            }
        };

        let parsed = match file.kind {
            SourceKind::DotEnv => {
                let priority = dotenv_priority(&file.path);
                DotEnvParser::new(priority).parse(&content, &file.path)
            }
            SourceKind::DockerCompose => DockerComposeParser.parse(&content, &file.path),
        };

        entries.extend(parsed);
    }

    entries
}
