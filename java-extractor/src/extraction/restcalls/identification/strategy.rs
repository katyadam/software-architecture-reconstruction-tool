use models::{CallStatement, RestCall};

pub trait IdentificationStrategy {
    fn identify_restcall(
        &self,
        call_statement: &CallStatement,
        file_path: &str,
    ) -> Option<RestCall>;
}
