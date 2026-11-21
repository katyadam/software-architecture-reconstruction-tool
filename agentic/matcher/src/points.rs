//! Service assignment and connection points for REST calls and endpoints.

use agentic_models::{Address, CodeElement, ConnectionPoint, ServiceId, Side};
use models::configuration::ServiceDescription;
use models::interaction_kind::host_of;
use models::{Endpoint, HttpMethod, RestCall};

/// A point with the service it belongs to and the target matching reads.
#[derive(Debug, Clone)]
pub struct Placed {
    pub point: ConnectionPoint,
    pub service: ServiceDescription,
    pub method: HttpMethod,
    /// Substituted call target, or the endpoint uri.
    pub target: String,
}

impl Placed {
    /// `target` is the call target after constant substitution.
    pub fn initiator(call: RestCall, service: ServiceDescription, target: String) -> Self {
        let host = Some(host_of(&target).to_string()).filter(|h| !h.is_empty());
        let address = Address::Http {
            method: Some(call.http_method),
            host,
            path: path_of(&target).to_string(),
        };
        let method = call.http_method;
        Self::new(
            Side::Initiator,
            CodeElement::RestCall(call),
            service,
            address,
            method,
            target,
        )
    }

    pub fn receiver(endpoint: Endpoint, service: ServiceDescription) -> Self {
        let address = Address::Http {
            method: Some(endpoint.http_method),
            host: None,
            path: endpoint.uri.clone(),
        };
        let (method, target) = (endpoint.http_method, endpoint.uri.clone());
        Self::new(
            Side::Receiver,
            CodeElement::Endpoint(endpoint),
            service,
            address,
            method,
            target,
        )
    }

    fn new(
        side: Side,
        element: CodeElement,
        service: ServiceDescription,
        address: Address,
        method: HttpMethod,
        target: String,
    ) -> Self {
        let point = ConnectionPoint {
            id: element.point_id(),
            service: ServiceId(service.name.clone()),
            side,
            protocol: element.protocol(),
            address,
            element,
        };
        Self {
            point,
            service,
            method,
            target,
        }
    }

    pub fn function_name(&self) -> &str {
        match &self.point.element {
            CodeElement::RestCall(c) => &c.function_name,
            CodeElement::Endpoint(e) => &e.function_name,
            CodeElement::Message(m) => &m.function_name,
        }
    }

    pub fn file(&self) -> &str {
        match &self.point.element {
            CodeElement::RestCall(c) => &c.file_path,
            CodeElement::Endpoint(e) => &e.file_path,
            CodeElement::Message(m) => &m.file_path,
        }
    }
}

/// `target` without scheme and authority; a relative target is kept whole.
fn path_of(target: &str) -> &str {
    match target.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("", |i| &rest[i..]),
        None => target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, base: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: base.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    #[test]
    fn address_splits_host_and_path() {
        let call = RestCall {
            target_uri: "{MDS}/v3/cases".into(),
            ..Default::default()
        };
        let p = Placed::initiator(call, svc("a", "a"), "http://mds:8000/v3/cases".into());
        let Address::Http { host, path, .. } = &p.point.address else {
            panic!("http address")
        };
        assert_eq!((host.as_deref(), path.as_str()), (Some("mds"), "/v3/cases"));
        let CodeElement::RestCall(c) = &p.point.element else {
            panic!("rest call")
        };
        assert_eq!(c.target_uri, "{MDS}/v3/cases");
    }

    #[test]
    fn relative_target_has_no_host() {
        let p = Placed::initiator(RestCall::default(), svc("a", "a"), "/v3/cases".into());
        let Address::Http { host, path, .. } = &p.point.address else {
            panic!("http address")
        };
        assert_eq!((host.as_deref(), path.as_str()), (None, "/v3/cases"));
    }
}
