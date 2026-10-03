//! Conservative Git-to-source change mapping for Java test impact analysis.

mod git_diff;
mod java_changes;
mod java_tests;

pub use git_diff::{ImpactError, analyze_changes};
pub use java_tests::discover_java_tests;
