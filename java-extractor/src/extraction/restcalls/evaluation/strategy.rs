use models::RestCall;

pub trait EvaluationStrategy {
    fn evaluate_restcall(&self, restcall: &RestCall) -> Option<RestCall>;
}
