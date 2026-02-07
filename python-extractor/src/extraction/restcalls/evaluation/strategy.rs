use models::RestCall;
use statix::error::EvalError;

pub trait EvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &RestCall) -> Result<Vec<RestCall>, EvalError>;
}
