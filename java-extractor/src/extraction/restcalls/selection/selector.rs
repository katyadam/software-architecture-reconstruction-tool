use models::{CallStatement, RestCall};
use statix::error::EvalError;

use crate::extraction::restcalls::{
    evaluation::strategy::EvaluationStrategy, identification::strategy::IdentificationStrategy,
};

pub trait Selector {
    fn identification_strategy(&self) -> &dyn IdentificationStrategy;
    fn evaluation_strategy(&self) -> &dyn EvaluationStrategy;

    fn identify(&self, call: &CallStatement, file_path: &str) -> Option<RestCall> {
        self.identification_strategy()
            .identify_restcall(call, file_path)
    }

    fn evaluate(&self, restcall: &mut RestCall) -> Result<(), EvalError> {
        self.evaluation_strategy().evaluate_restcall(restcall)
    }

    fn select_restcall_statements(
        &self,
        call_statements: &[CallStatement],
        file_path: &str,
    ) -> Result<Vec<RestCall>, EvalError> {
        Ok(call_statements
            .iter()
            .map(|call| {
                if let Some(mut restcall) = self.identify(call, file_path) {
                    self.evaluate(&mut restcall)?;
                    Ok(Some(restcall))
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<Option<RestCall>>, EvalError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<RestCall>>())
    }
}
