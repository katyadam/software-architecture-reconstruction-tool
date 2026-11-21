//! Configured services by name, for the data points carry only as `ServiceId`.

use std::collections::BTreeMap;

use agentic_models::ServiceId;
use models::configuration::ServiceDescription;

pub struct Services<'a> {
    /// Last config entry of a name wins, as the baseline's service map does.
    by_name: BTreeMap<&'a str, &'a ServiceDescription>,
    /// Every configured url, flattened, for `is_reflexive`.
    all_urls: Vec<String>,
}

impl<'a> Services<'a> {
    pub fn new(config: &'a [ServiceDescription]) -> Self {
        Self {
            by_name: config.iter().map(|sd| (sd.name.as_str(), sd)).collect(),
            all_urls: config.iter().flat_map(|sd| sd.urls.clone()).collect(),
        }
    }

    /// Configured urls; none for an unassigned (`""`) or unknown service.
    pub fn urls(&self, id: &ServiceId) -> &'a [String] {
        self.by_name
            .get(id.0.as_str())
            .map_or(&[], |sd| sd.urls.as_slice())
    }

    pub fn all_urls(&self) -> &[String] {
        &self.all_urls
    }
}
