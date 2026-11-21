use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_loop::adapters::Answers;
use agent_loop::{Answer, Log, Model, Outcome, Reply, Request, Session, Step};
use agentic_models::{
    ConnectionPoint, Fact, FactId, MatchSource, PointId, ServiceId, UnmatchedReason,
};
use agentic_tools::ToolCtx;
use extractor_runtime::pipeline::facts::test_fixtures::{ORIGIN, World, world};
use extractor_runtime::pipeline::pass3::OperandBindings;
use models::{Endpoint, HttpMethod, RestCall};

use super::outcome::{self, Resolved, Sessions};
use super::{JevCtx, JevFactory, Pick, Stage, answers, resolve, task};
use crate::plan::Pending;
use crate::points::{endpoint, rest_call};
use crate::services::Services;

const TARGET: &str = "http://mds:8000/v3/cases/7";

fn ep(service: &str, method: HttpMethod, uri: &str) -> ConnectionPoint {
    let e = Endpoint {
        http_method: method,
        uri: uri.into(),
        function_name: format!("h_{}", uri.len()),
        file_path: format!("/proj/{service}/api.py"),
        ..Default::default()
    };
    endpoint(e, ServiceId(service.into()))
}

fn call() -> ConnectionPoint {
    let rc = RestCall {
        function_name: "fetch_case".into(),
        function_hash: "h1".into(),
        http_method: HttpMethod::GET,
        target_uri: "self._mds_url + case_id".into(),
        file_path: ORIGIN.into(),
        ..Default::default()
    };
    rest_call(rc, ServiceId("caller".into()), TARGET)
}

fn pending(candidates: Vec<ConnectionPoint>) -> Pending {
    Pending {
        call: call(),
        target: TARGET.into(),
        candidates,
    }
}

fn bindings() -> OperandBindings {
    BTreeMap::from([("self._mds_url".to_string(), "settings.mds_url".to_string())])
}

fn mds() -> Vec<ConnectionPoint> {
    vec![
        ep("mds", HttpMethod::GET, "/v3/cases/{id}"),
        ep("mds", HttpMethod::POST, "/v3/cases"),
    ]
}

// --- task text ---

#[test]
fn task_lists_bindings_and_candidates() {
    let eps = mds();
    let p = pending(vec![eps[0].clone()]);
    let text = task::service_task(&p, Some(&bindings()));
    let expected = format!(
        "call: GET self._mds_url + case_id\n\
         substituted: {TARGET}\n\
         from: caller, {ORIGIN}, fetch_case\n\
         binding: self._mds_url = settings.mds_url\n\
         candidate: {} | mds | GET /v3/cases/{{id}}",
        eps[0].id.0
    );
    assert_eq!(text, expected);
    let endpoint = task::endpoint_task(&text, "mds");
    assert!(endpoint.ends_with("\npicked service: mds"));
}

#[test]
fn task_without_candidate() {
    let text = task::service_task(&pending(vec![]), None);
    assert!(text.ends_with("\ncandidate: none"), "{text}");
    assert!(!text.contains("binding:"));
}

// --- answer sets ---

fn labels(a: &Answers<Pick>) -> Vec<&str> {
    a.options.iter().map(|o| o.label.as_str()).collect()
}

#[test]
fn services_need_a_receiver_of_the_protocol() {
    let w = world();
    let eps = mds();
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let a = answers::services(&call(), &w.config.service_descriptions, &receivers);
    assert_eq!(labels(&a), vec!["mds"]);
    assert_eq!(a.options[0].description, "2 Http receivers");
    assert_eq!(a.none, None);
}

#[test]
fn endpoints_filter_method() {
    let eps = mds();
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let a = answers::endpoints(&call(), "mds", &receivers);
    assert_eq!(labels(&a), vec!["e1"]);
    assert_eq!(a.options[0].value, Some(eps[0].id.0.clone()));
    assert!(
        answers::endpoints(&call(), "auth", &receivers)
            .options
            .is_empty()
    );
}

#[test]
fn endpoints_capped_to_closest() {
    let mut eps: Vec<ConnectionPoint> = (0..300)
        .map(|i| ep("mds", HttpMethod::GET, &format!("/zz/q{i:03}/w")))
        .collect();
    eps.push(ep("mds", HttpMethod::GET, "/v3/cases/{id}"));
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let a = answers::endpoints(&call(), "mds", &receivers);
    assert_eq!(a.options.len(), 254);
    let close = Some(eps[300].id.0.clone());
    assert!(a.options.iter().any(|o| o.value == close));
}

// --- outcome table ---

fn session(outcome: Outcome<Pick>, facts: usize) -> Session<Pick> {
    let fact = |i: usize| Fact {
        id: FactId(format!("f{i}")),
        tool: "similar".into(),
        args: String::new(),
        text: format!("row {i}"),
    };
    Session {
        outcome,
        facts: (1..=facts).map(fact).collect(),
        calls: facts + 1,
        input_tokens: 0,
        output_tokens: 0,
        replayed: false,
    }
}

fn answer(value: Option<&str>, confidence: f64) -> Outcome<Pick> {
    Outcome::Answer(Answer {
        value: value.map(str::to_string),
        confidence: Some(confidence),
        note: value.is_none().then(|| "below 0.5: mds 0.3".to_string()),
    })
}

/// Resolves one point from its two session outcomes.
fn decide(service: Outcome<Pick>, endpoint: Option<Outcome<Pick>>) -> Resolved {
    let w = world();
    let eps = mds();
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let p = pending(vec![eps[0].clone(), eps[1].clone()]);
    let service = session(service, 2);
    let endpoint = endpoint.map(|o| session(o, 1));
    let s = Sessions {
        service: &service,
        endpoint: endpoint.as_ref(),
        receivers: &receivers,
    };
    outcome::resolve(&p, &s, &Services::new(&w.config.service_descriptions))
}

fn unmatched_reason(r: Resolved) -> (UnmatchedReason, String) {
    match r {
        Resolved::Unmatched(u) => (u.reason, u.note),
        Resolved::Link(..) => panic!("expected unmatched"),
    }
}

#[test]
fn endpoint_picked_links_receiver_with_renumbered_evidence() {
    let id = mds()[0].id.0.clone();
    let r = decide(answer(Some("mds"), 0.9), Some(answer(Some(&id), 0.8)));
    let Resolved::Link(target, link) = r else {
        panic!("expected link");
    };
    assert_eq!(target, ServiceId("mds".into()));
    assert_eq!(link.receiver.map(|r| r.id.0), Some(id));
    assert_eq!(
        (link.match_source, link.confidence),
        (MatchSource::Jev, Some(0.8))
    );
    let ids: Vec<_> = link.evidence.iter().map(|f| f.id.0.as_str()).collect();
    assert_eq!(ids, vec!["f1", "f2", "f3"]);
    assert_eq!(link.evidence[2].text, "row 1");
}

#[test]
fn endpoint_none_or_budget_or_skipped_links_service() {
    for endpoint in [
        Some(answer(None, 0.3)),
        Some(Outcome::BudgetExhausted),
        None,
    ] {
        let Resolved::Link(target, link) = decide(answer(Some("mds"), 0.9), endpoint) else {
            panic!("expected link");
        };
        assert_eq!(target, ServiceId("mds".into()));
        assert!(link.receiver.is_none());
        assert_eq!(link.confidence, Some(0.9));
    }
}

#[test]
fn own_service_link_is_reflexive() {
    let Resolved::Link(_, link) = decide(answer(Some("caller"), 0.9), None) else {
        panic!("expected link");
    };
    assert_eq!(link.kind, agentic_models::InteractionKind::Reflexive);
    let Resolved::Link(_, other) = decide(answer(Some("mds"), 0.9), None) else {
        panic!("expected link");
    };
    assert_eq!(other.kind, agentic_models::InteractionKind::Business);
}

#[test]
fn unknown_endpoint_id_keeps_service_confidence() {
    let r = decide(answer(Some("mds"), 0.9), Some(answer(Some("gone"), 0.8)));
    let Resolved::Link(_, link) = r else {
        panic!("expected link");
    };
    assert!(link.receiver.is_none());
    assert_eq!(link.confidence, Some(0.9));
}

#[test]
fn endpoint_model_error_is_unmatched() {
    let r = decide(
        answer(Some("mds"), 0.9),
        Some(Outcome::ModelError("jev: 503".into())),
    );
    assert_eq!(
        unmatched_reason(r),
        (UnmatchedReason::ModelError, "jev: 503".into())
    );
}

#[test]
fn service_outcomes_unmatched() {
    let cases = [
        (
            answer(None, 0.3),
            UnmatchedReason::NoFit,
            "below 0.5: mds 0.3",
        ),
        (
            Outcome::BudgetExhausted,
            UnmatchedReason::BudgetExhausted,
            "",
        ),
        (
            Outcome::ModelError("boom".into()),
            UnmatchedReason::ModelError,
            "boom",
        ),
    ];
    for (outcome, reason, note) in cases {
        let r = decide(outcome, None);
        let Resolved::Unmatched(u) = r else {
            panic!("expected unmatched");
        };
        assert_eq!((u.reason, u.note.as_str()), (reason, note));
        assert_eq!(u.tried.len(), 2);
        assert_eq!(u.evidence.len(), 2);
    }
}

// --- end to end with a scripted model ---

/// Fetches the first offered fact once, then picks a label or fails.
struct Scripted<'a> {
    answers: Answers<Pick>,
    pick: Result<&'static str, &'static str>,
    calls: &'a AtomicUsize,
}

impl Model<Pick> for Scripted<'_> {
    fn id(&self) -> &str {
        "scripted"
    }

    async fn step(&mut self, req: &Request<'_>) -> Result<Reply<Pick>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let step = match (req.facts.is_empty(), req.options.first()) {
            (true, Some(o)) => Step::Call(o.id.clone()),
            _ => Step::Final(Answer {
                value: self.value()?,
                confidence: Some(0.8),
                note: None,
            }),
        };
        Ok(Reply {
            step,
            input_tokens: 10,
            output_tokens: 1,
        })
    }
}

impl Scripted<'_> {
    fn value(&self) -> Result<Pick, String> {
        let label = self.pick.map_err(str::to_string)?;
        if label == "none" {
            return Ok(None);
        }
        let found = self.answers.options.iter().find(|o| o.label == label);
        found
            .map(|o| o.value.clone())
            .ok_or(format!("no label {label}"))
    }
}

struct Script {
    service: Result<&'static str, &'static str>,
    endpoint: Result<&'static str, &'static str>,
    calls: AtomicUsize,
}

impl Script {
    fn new(
        service: Result<&'static str, &'static str>,
        endpoint: Result<&'static str, &'static str>,
    ) -> Self {
        Self {
            service,
            endpoint,
            calls: AtomicUsize::new(0),
        }
    }
}

impl JevFactory for Script {
    type M<'a> = Scripted<'a>;

    fn model(&self, stage: Stage, answers: Answers<Pick>) -> Scripted<'_> {
        let pick = match stage {
            Stage::Service => self.service,
            Stage::Endpoint => self.endpoint,
        };
        Scripted {
            answers,
            pick,
            calls: &self.calls,
        }
    }
}

fn temp_log(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("matcher-jev-{}-{name}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// Runs one pending point through `resolve` on `log`.
async fn run(w: &World, script: &Script, log: &Log) -> super::JevResult {
    run_with(w, mds(), script, log).await
}

async fn run_with(
    w: &World,
    eps: Vec<ConnectionPoint>,
    script: &Script,
    log: &Log,
) -> super::JevResult {
    let receivers: Vec<&ConnectionPoint> = eps.iter().collect();
    let p = pending(eps.clone());
    let bound: HashMap<PointId, OperandBindings> = HashMap::from([(p.call.id.clone(), bindings())]);
    let ctx = JevCtx {
        tools: ToolCtx {
            project: &w.ir,
            config: &w.config,
            constants: &w.env,
            receivers: &receivers,
        },
        bindings: &bound,
    };
    resolve(&[p], &ctx, script, log).await.expect("resolve")
}

#[tokio::test]
async fn end_to_end_outcomes() {
    let w = world();
    let cases = [
        (Ok("mds"), Ok("e1"), "receiver"),
        (Ok("mds"), Ok("none"), "service"),
        (Ok("none"), Ok("e1"), "NoFit"),
        (Err("jev: 503"), Ok("e1"), "ModelError"),
    ];
    let mut total = crate::JevMetrics::default();
    for (i, (service, endpoint, want)) in cases.into_iter().enumerate() {
        let log = Log::open(&temp_log(&format!("e2e-{i}"))).expect("log");
        let out = run(&w, &Script::new(service, endpoint), &log).await;
        let got = match (out.links.first(), out.unmatched.first()) {
            (Some((_, l)), _) if l.receiver.is_some() => "receiver".to_string(),
            (Some(_), _) => "service".to_string(),
            (_, Some(u)) => format!("{:?}", u.reason),
            _ => panic!("no outcome"),
        };
        assert_eq!(got, want);
        if want == "receiver" {
            let link = &out.links[0].1;
            let tools: Vec<_> = link.evidence.iter().map(|f| f.id.0.as_str()).collect();
            assert_eq!(tools, vec!["f1", "f2"]);
            assert_eq!(link.evidence[0].tool, "define");
        }
        let m = out.metrics;
        total.initiators += m.initiators;
        total.sessions += m.sessions;
        total.receiver_links += m.receiver_links;
        total.service_links += m.service_links;
        for (k, v) in m.unmatched {
            *total.unmatched.entry(k).or_default() += v;
        }
    }
    assert_eq!(total.initiators, 4);
    // Two sessions each for the receiver and service cases, one for the rest.
    assert_eq!(total.sessions, 6);
    assert_eq!((total.receiver_links, total.service_links), (1, 1));
    let reasons: Vec<_> = total.unmatched.into_iter().collect();
    assert_eq!(reasons, vec![("ModelError".into(), 1), ("NoFit".into(), 1)]);
}

#[tokio::test]
async fn rerun_replays_every_session() {
    let w = world();
    let path = temp_log("replay");
    let first = Script::new(Ok("mds"), Ok("e1"));
    let out = run(&w, &first, &Log::open(&path).expect("log")).await;
    assert!(first.calls.load(Ordering::SeqCst) > 0);
    let again = Script::new(Ok("mds"), Ok("e1"));
    let replay = run(&w, &again, &Log::open(&path).expect("log")).await;
    assert_eq!(again.calls.load(Ordering::SeqCst), 0);
    assert_eq!(replay.metrics.replayed, 2);
    assert_eq!(replay.links[0].1.evidence, out.links[0].1.evidence);
    assert_eq!(replay.metrics.input_tokens, out.metrics.input_tokens);
}

#[tokio::test]
async fn no_service_with_receivers_skips_jev() {
    let w = world();
    let script = Script::new(Ok("mds"), Ok("e1"));
    let log = Log::open(&temp_log("no-service")).expect("log");
    let out = run_with(&w, vec![], &script, &log).await;
    assert_eq!(script.calls.load(Ordering::SeqCst), 0);
    assert_eq!(out.unmatched[0].reason, UnmatchedReason::NoCandidate);
    assert_eq!(out.unmatched[0].note, "no service has Http receivers");
    assert_eq!(out.metrics.sessions, 0);
}
