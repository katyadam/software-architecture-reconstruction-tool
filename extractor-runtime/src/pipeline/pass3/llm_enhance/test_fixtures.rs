//! The facts fixture world, plus the residual as DDFA's prompt state.

use std::collections::BTreeMap;

use clients::typesafe::state::{Enclosing, Receiver, ResidualCallState};
use models::{RestCall, source_code::SourceSpan};

use crate::pipeline::facts::test_fixtures::{ORIGIN, SNIPPET, World, world};
use crate::pipeline::pass3::restcalls::OperandBindings;

pub struct DdfaWorld {
    pub facts: World,
    pub rc: RestCall,
    pub bindings: OperandBindings,
    pub state: ResidualCallState,
}

pub fn ddfa_world() -> DdfaWorld {
    DdfaWorld {
        facts: world(),
        rc: RestCall {
            function_name: "fetch_case".to_string(),
            function_hash: "h1".to_string(),
            call_arguments: vec![],
            http_method: Default::default(),
            target_uri: "self._mds_url + case_id".to_string(),
            file_path: ORIGIN.to_string(),
            source_span: SourceSpan::new(0, 0),
        },
        bindings: BTreeMap::from([
            ("self._mds_url".to_string(), "settings.mds_url".to_string()),
            ("case_id".to_string(), "unresolved: str".to_string()),
        ]),
        state: ResidualCallState {
            call: "self._client.get(self._mds_url + case_id)".to_string(),
            receiver: Some(Receiver {
                expr: "self._client".to_string(),
                datatype: Some("httpx.AsyncClient".to_string()),
            }),
            residual: "self._mds_url + case_id".to_string(),
            operand_bindings: BTreeMap::new(),
            enclosing: Enclosing {
                function: "fetch_case(self, case_id)".to_string(),
                class: Some("MdsClient".to_string()),
            },
            snippet: Some(SNIPPET.to_string()),
            imports: vec!["settings".to_string()],
            file: "proj/caller/client.py".to_string(),
            facts: vec![],
        },
    }
}

#[test]
fn facts_subject_is_ddfa_subject() {
    use super::residual_edge_filter::subject;
    let w = ddfa_world();
    assert_eq!(subject(&w.rc, &w.bindings, &w.state), w.facts.subject);
}
