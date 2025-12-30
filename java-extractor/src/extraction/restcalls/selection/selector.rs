use models::{CallStatement, RestCall};

use crate::extraction::restcalls::identification::strategy::Strategy;

pub trait Selector {
    fn strategy(&self) -> &dyn Strategy;

    fn select_restcall_statements(
        &self,
        call_statements: &[CallStatement],
        file_path: &str,
    ) -> Vec<RestCall> {
        call_statements
            .iter()
            .filter_map(|call| self.strategy().identify_restcall(call, file_path))
            .collect()
    }
}
