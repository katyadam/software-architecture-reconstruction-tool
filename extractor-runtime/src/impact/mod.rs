//! Conservative Git-to-source change mapping for Java test impact analysis.

mod git_diff;
mod java_changes;

pub use git_diff::{ImpactError, analyze_changes};
