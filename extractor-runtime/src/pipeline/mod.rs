pub mod pass1;
pub mod pass2;
pub mod pass3;
pub mod pass_attr;

pub use pass1::dispatch_syntactic;
pub use pass2::build_project_ir;
pub use pass3::evaluate;
