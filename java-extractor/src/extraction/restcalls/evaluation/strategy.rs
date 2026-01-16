use models::RestCall;
use statix::error::EvalError;

pub trait EvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &mut RestCall) -> Result<(), EvalError>;
}
