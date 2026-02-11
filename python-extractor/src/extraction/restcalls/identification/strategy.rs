use models::RestCall;

use crate::extraction::calls::PythonCallStatement;

pub trait IdentificationStrategy {
    fn identify_restcall(
        &self,
        call_statement: &PythonCallStatement,
        file_path: &str,
    ) -> Option<RestCall>;
}
