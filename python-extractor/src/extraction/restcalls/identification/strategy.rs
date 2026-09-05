use models::{CallStatement, RestCall};

pub trait IdentificationStrategy {
    fn identify_restcall(&self, call: &CallStatement, file_path: &str) -> Option<RestCall>;
}
