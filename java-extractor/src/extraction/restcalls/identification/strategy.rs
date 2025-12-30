use models::{CallStatement, RestCall};

pub trait Strategy {
    fn identify_restcall(
        &self,
        call_statement: &CallStatement,
        file_path: &str,
    ) -> Option<RestCall>;
}
