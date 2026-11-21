//! HTTP dependencies of `Matches` in the synthesizer's SDG JSON shape, which score.py reads.

use agentic_models::{CodeElement, ConnectionPoint, InteractionKind, Protocol, Sdg};
use models::{Endpoint, RestCall};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LegacySdg {
    pub services: Vec<agentic_models::Service>,
    pub connections: Vec<Connection>,
    /// REST only for now; kept so the shape matches.
    pub message_connections: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct Connection {
    pub source_id: String,
    pub target_id: String,
    pub requests: Vec<Request>,
    pub kind: InteractionKind,
}

#[derive(Debug, Serialize)]
pub struct Request {
    /// `None` when only the target service is known.
    pub endpoint: Option<Endpoint>,
    /// The original element, so test-path detection sees the real file.
    pub restcall: RestCall,
    pub kind: InteractionKind,
}

pub fn legacy(sdg: &Sdg) -> LegacySdg {
    let connections = sdg
        .dependencies
        .iter()
        .filter(|d| d.protocol == Protocol::Http)
        .map(|d| Connection {
            source_id: d.source.0.clone(),
            target_id: d.target.0.clone(),
            kind: d.kind,
            requests: d
                .links
                .iter()
                .filter_map(|l| {
                    Some(Request {
                        endpoint: l.receiver.as_ref().and_then(endpoint),
                        restcall: rest_call(&l.initiator)?,
                        kind: l.kind,
                    })
                })
                .collect(),
        })
        .collect();
    LegacySdg {
        services: sdg.services.clone(),
        connections,
        message_connections: vec![],
    }
}

fn endpoint(p: &ConnectionPoint) -> Option<Endpoint> {
    match &p.element {
        CodeElement::Endpoint(e) => Some(e.clone()),
        _ => None,
    }
}

fn rest_call(p: &ConnectionPoint) -> Option<RestCall> {
    match &p.element {
        CodeElement::RestCall(c) => Some(c.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentic_matcher::match_static;
    use models::configuration::ServiceDescription;
    use models::ir::evaluted::EvaluatedIR;
    use models::{ConfigurationData, HttpMethod};

    fn svc(name: &str) -> ServiceDescription {
        ServiceDescription {
            name: name.into(),
            base_dir_path: name.into(),
            urls: vec![format!("http://{name}:8000")],
            java_analysis: None,
        }
    }

    #[test]
    fn link_becomes_request_with_original_call() {
        let config = ConfigurationData {
            service_descriptions: vec![svc("a"), svc("mds")],
        };
        let call = RestCall {
            http_method: HttpMethod::GET,
            target_uri: "{MDS}/v3/cases".into(),
            file_path: "p/a/client.py".into(),
            ..Default::default()
        };
        let ep = Endpoint {
            http_method: HttpMethod::GET,
            uri: "/v3/cases".into(),
            file_path: "p/mds/api.py".into(),
            ..Default::default()
        };
        let constants = [("MDS".to_string(), "http://mds:8000".to_string())];
        let ir = EvaluatedIR {
            endpoints: vec![ep],
            restcalls: vec![call],
            ..Default::default()
        };
        let (matches, _) = match_static(&ir, &config, &constants).expect("matches");
        let v = serde_json::to_value(legacy(&matches.sdg)).expect("serializes");
        let conn = &v["connections"][0];
        assert_eq!(
            (conn["source_id"].as_str(), conn["target_id"].as_str()),
            (Some("a"), Some("mds"))
        );
        assert_eq!(conn["kind"], "Business");
        let req = &conn["requests"][0];
        assert_eq!(req["restcall"]["file_path"], "p/a/client.py");
        assert_eq!(req["restcall"]["target_uri"], "{MDS}/v3/cases");
        assert_eq!(req["endpoint"]["uri"], "/v3/cases");
        assert_eq!(v["message_connections"], serde_json::json!([]));
    }
}
