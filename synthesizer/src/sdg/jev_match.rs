//! Jev tie-break: prunes ambiguous REST call -> endpoint matches and picks
//! endpoints for unmatched calls whose host names one service.

use std::collections::{BTreeMap, HashSet};

use clients::{
    error::TypeSafeError,
    typesafe::{
        SystemOneRequest,
        question::{
            MAX_CHOICE_OPTIONS,
            resolve::{self, ENDPOINT, NONE, Opt},
        },
        response::{Answer, SystemOneResponse},
        state::{Enclosing, ResidualCallState},
    },
};
use log::warn;
use models::{Endpoint, RestCall};

use crate::sdg::{
    builder::{UnmatchedCall, interaction_kind_rollup_connection, request_kind},
    interaction_kind::{InteractionKind, host_of},
    matching::MatchTag,
    model::{Connection, Request, Sdg},
};

/// Jev usage totals of one [`tie_break`] run.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MatchStats {
    pub calls: usize,
    pub input_tokens: u64,
}

/// Prunes calls with >1 candidate endpoint; undecided calls keep all candidates.
/// Unmatched calls whose host is one service's get Jev-picked endpoints.
pub async fn tie_break<F>(
    sdg: &mut Sdg,
    unmatched: &[UnmatchedCall],
    min_p: f64,
    mut ask: F,
) -> MatchStats
where
    F: AsyncFnMut(&SystemOneRequest) -> Result<SystemOneResponse, TypeSafeError>,
{
    let mut stats = MatchStats::default();
    let dropped = prune_ambiguous(sdg, min_p, &mut ask, &mut stats).await;
    for (ci, connection) in sdg.connections.iter_mut().enumerate() {
        let mut ri = 0;
        connection.requests.retain(|_| {
            ri += 1;
            !dropped.contains(&(ci, ri - 1))
        });
    }

    for (source, target, request) in
        match_unmatched(sdg, unmatched, min_p, &mut ask, &mut stats).await
    {
        match sdg
            .connections
            .iter_mut()
            .find(|c| c.source_id == source && c.target_id == target)
        {
            Some(c) => c.requests.push(request),
            None => sdg.connections.push(Connection {
                source_id: source,
                target_id: target,
                requests: vec![request],
                kind: InteractionKind::default(),
            }),
        }
    }

    sdg.connections.retain(|c| !c.requests.is_empty());
    interaction_kind_rollup_connection(&mut sdg.connections);
    stats
}

/// Asks once per call with >=2 requests; tags kept ones `Jev`, returns the dropped `(connection, request)` indices.
async fn prune_ambiguous<F>(
    sdg: &mut Sdg,
    min_p: f64,
    ask: &mut F,
    stats: &mut MatchStats,
) -> HashSet<(usize, usize)>
where
    F: AsyncFnMut(&SystemOneRequest) -> Result<SystemOneResponse, TypeSafeError>,
{
    let mut groups: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for (ci, connection) in sdg.connections.iter().enumerate() {
        for (ri, request) in connection.requests.iter().enumerate() {
            match serde_json::to_string(&request.restcall) {
                Ok(key) => groups.entry(key).or_default().push((ci, ri)),
                Err(e) => warn!("[match-jev] unserializable restcall: {e}"),
            }
        }
    }

    let mut kept = Vec::new();
    let mut dropped = HashSet::new();
    for refs in groups.values_mut().filter(|g| g.len() >= 2) {
        let at = |&(ci, ri): &(usize, usize)| {
            let c: &Connection = &sdg.connections[ci];
            (&c.target_id, &c.requests[ri].endpoint)
        };
        refs.sort_by(|a, b| sort_key(at(a)).cmp(&sort_key(at(b))));
        let options: Vec<Opt> = refs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let (target, endpoint) = at(r);
                (format!("e{i}"), describe(target, endpoint))
            })
            .collect();
        let restcall = &sdg.connections[refs[0].0].requests[refs[0].1].restcall;
        let Some(picked) = choose(ask, restcall, &options, min_p, stats).await else {
            continue;
        };
        for (i, r) in refs.iter().enumerate() {
            if picked.contains(&i) {
                kept.push(*r);
            } else {
                dropped.insert(*r);
            }
        }
    }
    for (ci, ri) in kept {
        sdg.connections[ci].requests[ri].match_tag = MatchTag::Jev;
    }
    dropped
}

/// New `(source, target, request)` for unmatched calls whose host is exactly one other service's.
async fn match_unmatched<F>(
    sdg: &Sdg,
    unmatched: &[UnmatchedCall],
    min_p: f64,
    ask: &mut F,
    stats: &mut MatchStats,
) -> Vec<(String, String, Request)>
where
    F: AsyncFnMut(&SystemOneRequest) -> Result<SystemOneResponse, TypeSafeError>,
{
    let all_urls: Vec<String> = sdg.services.iter().flat_map(|s| s.urls.clone()).collect();
    let mut added = Vec::new();
    for call in unmatched {
        let host = host_of(&call.restcall.target_uri);
        if host.is_empty() {
            continue;
        }
        let owners: Vec<_> = sdg
            .services
            .iter()
            .filter(|s| {
                s.name != call.source
                    && s.urls.iter().any(|u| host_of(u).eq_ignore_ascii_case(host))
            })
            .collect();
        let [target] = owners.as_slice() else {
            continue;
        };
        let mut endpoints: Vec<&Endpoint> = target
            .endpoints
            .iter()
            .filter(|e| e.http_method == call.restcall.http_method)
            .collect();
        // `none` takes the last slot.
        if endpoints.is_empty() || endpoints.len() >= MAX_CHOICE_OPTIONS {
            continue;
        }
        endpoints.sort_by(|a, b| sort_key((&target.name, a)).cmp(&sort_key((&target.name, b))));
        let options: Vec<Opt> = endpoints
            .iter()
            .enumerate()
            .map(|(i, e)| (format!("e{i}"), describe(&target.name, e)))
            .collect();
        let Some(picked) = choose(ask, &call.restcall, &options, min_p, stats).await else {
            continue;
        };
        let own_urls = sdg
            .services
            .iter()
            .find(|s| s.name == call.source)
            .map_or(&[][..], |s| s.urls.as_slice());
        for i in picked {
            let endpoint = endpoints[i];
            added.push((
                call.source.clone(),
                target.name.clone(),
                Request {
                    endpoint: endpoint.clone(),
                    restcall: call.restcall.clone(),
                    kind: request_kind(&call.restcall, &endpoint.uri, own_urls, &all_urls),
                    match_tag: MatchTag::Jev,
                },
            ));
        }
    }
    added
}

/// Asks Jev to pick among `options`; indices with p >= `min_p`, `None` = keep as is.
async fn choose<F>(
    ask: &mut F,
    restcall: &RestCall,
    options: &[Opt],
    min_p: f64,
    stats: &mut MatchStats,
) -> Option<Vec<usize>>
where
    F: AsyncFnMut(&SystemOneRequest) -> Result<SystemOneResponse, TypeSafeError>,
{
    let site = format!("{}@{}", restcall.file_path, restcall.source_span.start_byte);
    let question = match resolve::endpoint(options) {
        Ok(q) => q,
        Err(e) => {
            warn!("[match-jev] {site}: {e}");
            return None;
        }
    };
    let request = SystemOneRequest::new(
        state(restcall),
        BTreeMap::from([(ENDPOINT.to_string(), question)]),
    );
    let response = match ask(&request).await {
        Ok(r) => r,
        Err(e) => {
            warn!("[match-jev] {site}: {e}, keeping candidates");
            return None;
        }
    };
    stats.calls += 1;
    stats.input_tokens += response.usage.input_tokens;

    let mut top = 0.0;
    let picked = match response.answers.get(ENDPOINT) {
        Some(Answer::Choice {
            choice,
            probabilities,
            ..
        }) => {
            top = probabilities.values().copied().fold(0.0, f64::max);
            let p = |id: &str| probabilities.get(id).copied().unwrap_or(0.0);
            let kept: Vec<usize> = options
                .iter()
                .enumerate()
                .filter(|(_, (id, _))| p(id) >= min_p)
                .map(|(i, _)| i)
                .collect();
            Some(kept).filter(|k| choice != NONE && !k.is_empty())
        }
        _ => None,
    };
    println!(
        "[match-jev] {site} {}/{} {top:.2} {}",
        picked.as_ref().map_or(options.len(), Vec::len),
        options.len(),
        response.usage.input_tokens
    );
    if picked.is_none() {
        warn!("[match-jev] {site}: none or below {min_p}, keeping candidates");
    }
    picked
}

fn state(restcall: &RestCall) -> ResidualCallState {
    ResidualCallState {
        call: format!("{:?} {}", restcall.http_method, restcall.target_uri),
        receiver: None,
        residual: restcall.target_uri.clone(),
        operand_bindings: BTreeMap::new(),
        enclosing: Enclosing {
            function: restcall.function_name.clone(),
            class: None,
        },
        snippet: None,
        imports: vec![],
        file: restcall.file_path.clone(),
        facts: vec![],
    }
}

/// `<METHOD> <service> <uri> -> <function>(<name>: <type>, ...)`.
fn describe(service: &str, endpoint: &Endpoint) -> String {
    let params = endpoint
        .parameters
        .iter()
        .map(|p| format!("{}: {}", p.name, p.datatype.as_deref().unwrap_or("?")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{:?} {service} {} -> {}({params})",
        endpoint.http_method, endpoint.uri, endpoint.function_name
    )
}

/// Deterministic option order: service, uri, function, file.
fn sort_key<'a>(
    (service, e): (&'a String, &'a Endpoint),
) -> (&'a String, &'a String, &'a String, &'a String) {
    (service, &e.uri, &e.function_name, &e.file_path)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use clients::typesafe::{
        question::resolve::{ENDPOINT, NONE},
        response::{Answer, Usage},
    };
    use models::{Endpoint, HttpMethod, RestCall};

    use super::*;
    use crate::sdg::{
        interaction_kind::InteractionKind,
        matching::MatchTag,
        model::{Connection, Request, Service},
    };

    fn ep(method: HttpMethod, uri: &str) -> Endpoint {
        Endpoint {
            http_method: method,
            uri: uri.into(),
            function_name: format!("handle{}", uri.replace('/', "_")),
            ..Default::default()
        }
    }

    fn rc(target: &str) -> RestCall {
        RestCall {
            function_name: "caller".into(),
            target_uri: target.into(),
            file_path: "a/src/Client.java".into(),
            ..Default::default()
        }
    }

    fn svc(name: &str, port: u16, endpoints: Vec<Endpoint>) -> Service {
        Service {
            name: name.into(),
            endpoints,
            urls: vec![format!("http://{name}:{port}")],
        }
    }

    fn conn(target: &str, uris: &[&str], call: &RestCall) -> Connection {
        Connection {
            source_id: "a".into(),
            target_id: target.into(),
            requests: uris
                .iter()
                .map(|u| Request {
                    endpoint: ep(HttpMethod::GET, u),
                    restcall: call.clone(),
                    kind: InteractionKind::Business,
                    match_tag: MatchTag::Template,
                })
                .collect(),
            kind: InteractionKind::Business,
        }
    }

    /// One call, three candidates in `b`.
    fn ambiguous() -> Sdg {
        let call = rc("http://b:2/x/{id}");
        Sdg {
            services: vec![svc("a", 1, vec![]), svc("b", 2, vec![])],
            connections: vec![conn("b", &["/x/1", "/x/2", "/x/3"], &call)],
            message_connections: vec![],
        }
    }

    fn reply(choice: &str, probs: &[(&str, f64)]) -> Result<SystemOneResponse, TypeSafeError> {
        Ok(SystemOneResponse {
            model: "jev-1.13.0".into(),
            answers: HashMap::from([(
                ENDPOINT.to_string(),
                Answer::Choice {
                    choice: choice.into(),
                    probabilities: probs.iter().map(|(k, p)| (k.to_string(), *p)).collect(),
                    confidence: 0.9,
                },
            )]),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 1,
            },
        })
    }

    /// Runs `tie_break` with one scripted answer; returns stats and the requests sent.
    async fn run(
        sdg: &mut Sdg,
        unmatched: &[UnmatchedCall],
        answer: Result<SystemOneResponse, TypeSafeError>,
    ) -> (MatchStats, Vec<serde_json::Value>) {
        let mut answer = Some(answer);
        let mut asked = Vec::new();
        let stats = tie_break(sdg, unmatched, 0.2, async |req: &SystemOneRequest| {
            asked.push(serde_json::to_value(req).expect("json"));
            answer.take().expect("asked once")
        })
        .await;
        (stats, asked)
    }

    fn uris_and_tags(sdg: &Sdg) -> Vec<(String, String, MatchTag)> {
        let mut out: Vec<_> = sdg
            .connections
            .iter()
            .flat_map(|c| {
                c.requests
                    .iter()
                    .map(|r| (c.target_id.clone(), r.endpoint.uri.clone(), r.match_tag))
            })
            .collect();
        out.sort();
        out
    }

    #[tokio::test]
    async fn keeps_candidates_above_threshold() {
        let mut sdg = ambiguous();
        let (stats, asked) = run(
            &mut sdg,
            &[],
            reply("e0", &[("e0", 0.7), ("e1", 0.25), ("e2", 0.05)]),
        )
        .await;
        assert_eq!(
            uris_and_tags(&sdg),
            vec![
                ("b".into(), "/x/1".into(), MatchTag::Jev),
                ("b".into(), "/x/2".into(), MatchTag::Jev),
            ]
        );
        assert_eq!(
            stats,
            MatchStats {
                calls: 1,
                input_tokens: 100
            }
        );
        assert_eq!(asked[0]["state"]["call"], "GET http://b:2/x/{id}");
        assert_eq!(asked[0]["state"]["enclosing"]["function"], "caller");
        let criteria = asked[0]["questions"][ENDPOINT]["criteria"]
            .as_object()
            .expect("choice criteria");
        assert_eq!(criteria.len(), 4);
    }

    #[tokio::test]
    async fn none_keeps_all() {
        let mut sdg = ambiguous();
        run(
            &mut sdg,
            &[],
            reply(NONE, &[(NONE, 0.9), ("e0", 0.05), ("e1", 0.05)]),
        )
        .await;
        assert_eq!(sdg, ambiguous());
    }

    #[tokio::test]
    async fn jev_error_keeps_all() {
        let mut sdg = ambiguous();
        let err = Err(TypeSafeError::Status {
            status: 500,
            body: String::new(),
        });
        run(&mut sdg, &[], err).await;
        assert_eq!(sdg, ambiguous());
    }

    #[tokio::test]
    async fn single_candidate_not_asked() {
        let call = rc("http://b:2/x/1");
        let mut sdg = Sdg {
            services: vec![svc("a", 1, vec![]), svc("b", 2, vec![])],
            connections: vec![conn("b", &["/x/1"], &call)],
            message_connections: vec![],
        };
        let before = uris_and_tags(&sdg);
        let stats = tie_break(&mut sdg, &[], 0.2, async |_: &SystemOneRequest| {
            panic!("must not ask")
        })
        .await;
        assert_eq!(stats, MatchStats::default());
        assert_eq!(uris_and_tags(&sdg), before);
    }

    #[tokio::test]
    async fn empty_connection_removed() {
        let call = rc("http://x/x/{id}");
        let mut sdg = Sdg {
            services: vec![
                svc("a", 1, vec![]),
                svc("b", 2, vec![]),
                svc("c", 3, vec![]),
            ],
            // Unsorted on purpose: options follow (target, uri), so e0 = b.
            connections: vec![conn("c", &["/x/2"], &call), conn("b", &["/x/1"], &call)],
            message_connections: vec![],
        };
        run(&mut sdg, &[], reply("e0", &[("e0", 0.9), ("e1", 0.1)])).await;
        assert_eq!(sdg.connections.len(), 1);
        assert_eq!(
            uris_and_tags(&sdg),
            vec![("b".into(), "/x/1".into(), MatchTag::Jev)]
        );
    }

    #[tokio::test]
    async fn unmatched_known_host_adds_jev_request() {
        let mut sdg = Sdg {
            services: vec![
                svc("a", 1, vec![]),
                svc(
                    "b",
                    2,
                    vec![
                        ep(HttpMethod::GET, "/q"),
                        ep(HttpMethod::POST, "/r"),
                        ep(HttpMethod::GET, "/p/{id}"),
                    ],
                ),
            ],
            connections: vec![],
            message_connections: vec![],
        };
        let unmatched = [
            UnmatchedCall {
                source: "a".into(),
                restcall: rc("http://B:2/zzz"),
            },
            // Unknown host: not asked.
            UnmatchedCall {
                source: "a".into(),
                restcall: rc("http://elsewhere/zzz"),
            },
        ];
        let (stats, asked) = run(
            &mut sdg,
            &unmatched,
            reply("e1", &[("e0", 0.1), ("e1", 0.8), (NONE, 0.1)]),
        )
        .await;
        assert_eq!(stats.calls, 1);
        // GET /p/{id}, GET /q + none.
        let criteria = asked[0]["questions"][ENDPOINT]["criteria"]
            .as_object()
            .expect("choice criteria");
        assert_eq!(criteria.len(), 3);
        assert_eq!(
            uris_and_tags(&sdg),
            vec![("b".into(), "/q".into(), MatchTag::Jev)]
        );
        let c = &sdg.connections[0];
        assert_eq!(
            (c.source_id.as_str(), c.kind),
            ("a", InteractionKind::Business)
        );
        assert_eq!(c.requests[0].restcall.target_uri, "http://B:2/zzz");
    }
}
