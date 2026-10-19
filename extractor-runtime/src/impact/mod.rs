//! Conservative Git-to-source change mapping for Java test impact analysis.

mod git_diff;
mod java_changes;
mod java_modules;
mod java_tests;
mod module_dependencies;
mod selection;

pub use git_diff::{ImpactError, analyze_changes};
pub use java_modules::ModuleCatalog;
pub use java_tests::discover_java_tests;
pub use module_dependencies::ModuleDependencyGraph;
pub use selection::{
    include_potential_consumer_tests, select_tests, select_tests_with_reachability,
};
