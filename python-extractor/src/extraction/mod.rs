pub mod assignments;
pub mod callables;
pub mod calls;
pub mod common;
pub mod endpoints;
pub mod entities;
pub mod enums;
pub mod extractor;
pub mod imports;
pub mod message_edges;
pub mod module;
pub mod parse;
mod post_process;
pub mod queries;
pub mod restcalls;

/// Applies Python-specific Pass 2 transformations without exposing framework
/// details to the extractor runtime.
pub fn post_process(files: &mut [&mut models::ir::project::TypedFileRecord]) {
    post_process::post_process(files);
}
