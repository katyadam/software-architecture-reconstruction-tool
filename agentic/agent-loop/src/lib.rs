//! Generic model loop: the model calls a tool or answers; the runtime records
//! tool output as numbered facts and logs finished sessions for resume.

mod log;
mod protocol;
mod session;

pub use log::{Log, prompt_hash};
pub use protocol::{Model, Reply, Request, Step, ToolOption, Tools};
pub use session::{Outcome, Session, SessionKey, run_session};

#[cfg(test)]
mod tests;
