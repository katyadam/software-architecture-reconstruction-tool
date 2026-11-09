//! Conservative Git-to-source change mapping for Java test impact analysis.

mod git_diff;
pub mod go;
mod go_changes;
mod go_git;
mod go_package;
mod go_package_changes;
pub mod go_package_selection;
mod go_selection;
mod java_changes;
mod java_modules;
mod java_tests;
mod module_dependencies;
pub mod python;
mod python_changes;
mod python_graph;
mod python_tests;
mod selection;

pub use git_diff::{ImpactError, analyze_changes, analyze_changes_with_catalog};
pub use java_modules::ModuleCatalog;
pub use java_tests::{
    checkout_modules, discover_java_tests, discover_java_tests_in_checkout,
    discover_java_tests_in_roots,
};
pub use module_dependencies::ModuleDependencyGraph;
pub use selection::{
    include_potential_consumer_tests, select_tests, select_tests_with_reachability,
};
