pub mod files;
pub mod pass1;
pub mod pass2;
pub mod pass3;

pub use files::{collect_files, collect_syntactic_records};
pub use pass1::dispatch_syntactic;
pub use pass2::build_project_ir;
pub use pass3::evaluate;
