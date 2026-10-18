//! Native Go impact analysis, separate from the Java command and identities.
pub use super::go_changes::map_go_changes;
pub use super::go_git::{GoFileDiff, GoGitDiff, GoHunk, read_go_diff};
#[derive(Debug, thiserror::Error)]
pub enum GoImpactError {
    #[error("Invalid Go impact input: {0}")]
    Input(String),
    #[error("Go impact Git failure: {0}")]
    Git(String),
    #[error("Go impact output failure: {0}")]
    Output(String),
}
