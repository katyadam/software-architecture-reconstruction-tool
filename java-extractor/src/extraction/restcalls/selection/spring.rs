use crate::extraction::restcalls::{
    identification::{spring::SpringStrategy, strategy::Strategy},
    selection::selector::Selector,
};

pub struct SpringSelector {
    strategy: SpringStrategy,
}

impl<'a> SpringSelector {
    pub fn new(strategy: SpringStrategy) -> Self {
        Self { strategy }
    }
}

impl<'a> Selector for SpringSelector {
    fn strategy(&self) -> &dyn Strategy {
        &self.strategy
    }
}
