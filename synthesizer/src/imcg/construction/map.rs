use std::collections::HashMap;

use crate::imcg::model::ServiceCallable;

pub fn get_callables_map(callables: &[ServiceCallable]) -> HashMap<String, ServiceCallable> {
    let mut map = HashMap::with_capacity(callables.len());
    for c in callables {
        map.insert(c.callable.hash.clone(), c.clone());
    }
    map
}
