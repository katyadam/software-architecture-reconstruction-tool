use crate::extraction::restcalls::{
    evaluation::{spring::SpringEvaluationStrategy, strategy::EvaluationStrategy},
    identification::{spring::SpringIdentificationStrategy, strategy::IdentificationStrategy},
    selection::selector::Selector,
};

pub struct SpringSelector {
    identification_strategy: SpringIdentificationStrategy,
    evaluation_strategy: SpringEvaluationStrategy,
}

impl SpringSelector {
    pub fn new(
        identification_strategy: SpringIdentificationStrategy,
        evaluation_strategy: SpringEvaluationStrategy,
    ) -> Self {
        Self {
            identification_strategy,
            evaluation_strategy,
        }
    }
}

impl Selector for SpringSelector {
    fn identification_strategy(&self) -> &dyn IdentificationStrategy {
        &self.identification_strategy
    }

    fn evaluation_strategy(&self) -> &dyn EvaluationStrategy {
        &self.evaluation_strategy
    }
}
