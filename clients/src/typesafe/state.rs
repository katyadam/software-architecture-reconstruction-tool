use std::collections::BTreeMap;

use serde::Serialize;

/// `state` of a System One call classifying a residual REST call site:
/// is it a cross-service HTTP edge, or noise (DB/dict `.get`, third-party API)?
///
/// Field order = serialization order; maps are `BTreeMap` so the JSON is
/// stable and can be hashed as a decision-cache key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResidualCallState {
    /// Call as written, e.g. `self._client.get(url, timeout=10)`.
    pub call: String,
    /// Omitted when the callee has no receiver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<Receiver>,
    /// `target_uri` left after symbolic evaluation.
    pub residual: String,
    /// Residual operand -> where its value comes from,
    /// e.g. `self._mds_url` -> `settings.mds_url`, `case_id` -> `parameter: case_id: str`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub operand_bindings: BTreeMap<String, String>,
    pub enclosing: Enclosing,
    /// Imports of the call's file, e.g. `aiohttp`, `asyncpg.exceptions.PostgresError`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// Source file path, relative to the project root.
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Receiver {
    /// Object the HTTP-named method is called on, e.g. `self._client`.
    pub expr: String,
    /// Inferred type, e.g. `httpx.AsyncClient`; omitted when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datatype: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Enclosing {
    /// Enclosing function signature, e.g. `get_case(self, case_id: str)`.
    pub function: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_stable_and_omits_unknowns() {
        let state = ResidualCallState {
            call: "self._client.get(url)".to_string(),
            receiver: Some(Receiver {
                expr: "self._client".to_string(),
                datatype: None,
            }),
            residual: "self._mds_url + url".to_string(),
            operand_bindings: BTreeMap::from([
                ("url".to_string(), "parameter: url: str".to_string()),
                ("self._mds_url".to_string(), "settings.mds_url".to_string()),
            ]),
            enclosing: Enclosing {
                function: "get(self, url: str)".to_string(),
                class: Some("MdsClient".to_string()),
            },
            imports: vec!["httpx".to_string()],
            file: "wbs/clients/mds.py".to_string(),
        };
        let json = serde_json::to_string(&state).expect("serializable");
        assert_eq!(
            json,
            r#"{"call":"self._client.get(url)","receiver":{"expr":"self._client"},"residual":"self._mds_url + url","operand_bindings":{"self._mds_url":"settings.mds_url","url":"parameter: url: str"},"enclosing":{"function":"get(self, url: str)","class":"MdsClient"},"imports":["httpx"],"file":"wbs/clients/mds.py"}"#
        );
    }
}
