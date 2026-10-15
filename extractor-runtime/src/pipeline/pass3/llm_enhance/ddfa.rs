//! Demand-Driven Fact Acquisition (DDFA): Jev picks the target service of a
//! residual, acquiring facts on demand until it has enough.

use std::collections::HashMap;

use clients::{
    error::TypeSafeError,
    typesafe::{
        SystemOneRequest,
        question::resolve::{self, ENOUGH, NEXT, NONE, Opt, STOP, TARGET},
        response::{Answer, SystemOneResponse},
    },
};
use models::configuration::ServiceDescription;

use super::facts::{Explored, FactCtx, available, fetch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdfaMode {
    /// Fetch every first-hop fact, ask once. Baseline.
    Once,
    /// Ask, fetch the fact Jev picks, repeat.
    Acquire,
}

pub(super) struct Limits {
    /// Max fetches in `Acquire`.
    pub max_steps: usize,
    /// `enough` at or above this -> decide.
    pub enough: f64,
    /// Min probability of the chosen service.
    pub target_min: f64,
    /// Facts fetched in `Once`.
    pub once_facts: usize,
}

impl Limits {
    /// `DDFA_MAX_STEPS`, `DDFA_ENOUGH`, `DDFA_TARGET_MIN`, `DDFA_ONCE_FACTS` override.
    pub(super) fn from_env() -> Self {
        Self {
            max_steps: env_or("DDFA_MAX_STEPS", 6),
            enough: env_or("DDFA_ENOUGH", 0.7),
            target_min: env_or("DDFA_TARGET_MIN", 0.5),
            once_facts: env_or("DDFA_ONCE_FACTS", 12),
        }
    }
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[derive(Debug)]
pub(super) struct Resolution {
    /// Chosen service name; `None` = abstain.
    pub service: Option<String>,
    pub calls: usize,
    pub input_tokens: u64,
    /// Option ids fetched, in order.
    pub trail: Vec<String>,
}

pub(super) async fn resolve<F>(
    mode: DdfaMode,
    ctx: &FactCtx<'_>,
    services: &[&ServiceDescription],
    limits: &Limits,
    mut ask: F,
) -> Result<Resolution, TypeSafeError>
where
    F: AsyncFnMut(&SystemOneRequest) -> Result<SystemOneResponse, TypeSafeError>,
{
    let targets = target_options(services);
    let mut state = ctx.state.clone();
    let mut explored = Explored::new(&ctx.origin.file_path);
    let mut out = Resolution {
        service: None,
        calls: 0,
        input_tokens: 0,
        trail: vec![],
    };

    if mode == DdfaMode::Once {
        for action in available(ctx, &explored)
            .into_iter()
            .take(limits.once_facts)
        {
            let fact = fetch(ctx, &action, &mut explored);
            out.trail.push(fact.action.clone());
            state.facts.push(fact);
        }
        let response = ask(&SystemOneRequest::new(
            state,
            resolve::acquire_step(&targets, &[])?,
        ))
        .await?;
        out.calls = 1;
        out.input_tokens = response.usage.input_tokens;
        out.service = chosen_target(&response.answers, limits.target_min);
        return Ok(out);
    }

    let mut step = 0;
    loop {
        let actions = available(ctx, &explored);
        let options: Vec<Opt> = actions.iter().map(|a| a.option(ctx)).collect();
        let request =
            SystemOneRequest::new(state.clone(), resolve::acquire_step(&targets, &options)?);
        let response = ask(&request).await?;
        out.calls += 1;
        out.input_tokens += response.usage.input_tokens;

        let picked = match next_move(&response.answers, limits, step) {
            Move::Fetch(id) => actions
                .iter()
                .zip(&options)
                .find(|(_, (option_id, _))| *option_id == id)
                .map(|(action, _)| action),
            Move::Decide => None,
        };
        let Some(action) = picked else {
            out.service = chosen_target(&response.answers, limits.target_min);
            return Ok(out);
        };
        let fact = fetch(ctx, action, &mut explored);
        out.trail.push(fact.action.clone());
        state.facts.push(fact);
        step += 1;
    }
}

/// `(name, "url (code in base_dir)")` per candidate service.
fn target_options(services: &[&ServiceDescription]) -> Vec<Opt> {
    services
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                format!(
                    "{} (code in {})",
                    s.urls.first().map_or("", String::as_str),
                    s.base_dir_path
                ),
            )
        })
        .collect()
}

enum Move {
    Fetch(String),
    Decide,
}

/// Decide when enough, stopped, out of budget, or `next` missing.
fn next_move(answers: &HashMap<String, Answer>, limits: &Limits, step: usize) -> Move {
    if step >= limits.max_steps {
        return Move::Decide;
    }
    if let Some(Answer::Noul { noul }) = answers.get(ENOUGH)
        && *noul >= limits.enough
    {
        return Move::Decide;
    }
    match answers.get(NEXT) {
        Some(Answer::Choice { choice, .. }) if choice != STOP => Move::Fetch(choice.clone()),
        _ => Move::Decide,
    }
}

/// Chosen service unless `none` or below `min`.
fn chosen_target(answers: &HashMap<String, Answer>, min: f64) -> Option<String> {
    match answers.get(TARGET)? {
        Answer::Choice {
            choice,
            probabilities,
            ..
        } if choice != NONE && probabilities.get(choice).copied().unwrap_or(0.0) >= min => {
            Some(choice.clone())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::pass3::llm_enhance::test_fixtures::world;
    use clients::typesafe::response::Usage;

    const LIMITS: Limits = Limits {
        max_steps: 6,
        enough: 0.7,
        target_min: 0.5,
        once_facts: 12,
    };
    const OPEN_SETTINGS: &str = "open:proj/caller/settings.py#settings";

    fn choice(c: &str, p: f64) -> Answer {
        Answer::Choice {
            choice: c.to_string(),
            probabilities: HashMap::from([(c.to_string(), p)]),
            confidence: p,
        }
    }

    fn response(answers: Vec<(&str, Answer)>) -> SystemOneResponse {
        SystemOneResponse {
            model: "jev-1.13.0".to_string(),
            answers: answers
                .into_iter()
                .map(|(k, a)| (k.to_string(), a))
                .collect(),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 1,
            },
        }
    }

    fn step(enough: f64, next: &str, target: &str, p: f64) -> SystemOneResponse {
        response(vec![
            (ENOUGH, Answer::Noul { noul: enough }),
            (NEXT, choice(next, 0.9)),
            (TARGET, choice(target, p)),
        ])
    }

    /// Runs `resolve` against scripted answers; returns it plus each request as JSON.
    async fn run(
        mode: DdfaMode,
        limits: &Limits,
        script: Vec<Result<SystemOneResponse, TypeSafeError>>,
    ) -> (Result<Resolution, TypeSafeError>, Vec<serde_json::Value>) {
        let w = world();
        let ctx = w.ctx();
        let services: Vec<&ServiceDescription> = w
            .config
            .service_descriptions
            .iter()
            .filter(|s| !s.urls.is_empty())
            .collect();
        let mut script = script.into_iter();
        let mut asked = Vec::new();
        let out = resolve(
            mode,
            &ctx,
            &services,
            limits,
            async |req: &SystemOneRequest| {
                asked.push(serde_json::to_value(req).expect("json"));
                script.next().expect("scripted response")
            },
        )
        .await;
        (out, asked)
    }

    #[tokio::test]
    async fn acquires_until_enough() {
        let (out, asked) = run(
            DdfaMode::Acquire,
            &LIMITS,
            vec![
                Ok(step(0.2, OPEN_SETTINGS, NONE, 0.6)),
                Ok(step(0.9, STOP, "mds", 0.8)),
            ],
        )
        .await;
        let out = out.expect("resolves");
        assert_eq!(out.service.as_deref(), Some("mds"));
        assert_eq!((out.calls, out.input_tokens), (2, 200));
        assert_eq!(out.trail, vec![OPEN_SETTINGS]);
        assert!(asked[0]["state"].get("facts").is_none());
        assert_eq!(asked[1]["state"]["facts"][0]["action"], OPEN_SETTINGS);
        assert!(asked[0]["questions"].get(NEXT).is_some());
    }

    #[tokio::test]
    async fn unknown_next_id_decides() {
        let (out, asked) = run(
            DdfaMode::Acquire,
            &LIMITS,
            vec![Ok(step(0.1, "open:nope", "auth", 0.7))],
        )
        .await;
        let out = out.expect("resolves");
        assert_eq!(out.service.as_deref(), Some("auth"));
        assert_eq!(asked.len(), 1);
        assert!(out.trail.is_empty());
    }

    #[tokio::test]
    async fn budget_caps_fetches() {
        let limits = Limits {
            max_steps: 2,
            ..LIMITS
        };
        let (out, asked) = run(
            DdfaMode::Acquire,
            &limits,
            vec![
                Ok(step(0.1, OPEN_SETTINGS, NONE, 0.9)),
                Ok(step(0.1, "define:mds_url", NONE, 0.9)),
                Ok(step(0.1, "callers", "mds", 0.9)),
            ],
        )
        .await;
        let out = out.expect("resolves");
        assert_eq!(out.trail, vec![OPEN_SETTINGS, "define:mds_url"]);
        assert_eq!(asked.len(), 3);
        assert_eq!(out.service.as_deref(), Some("mds"));
    }

    #[tokio::test]
    async fn none_or_unsure_target_abstains() {
        let (none, _) = run(
            DdfaMode::Acquire,
            &LIMITS,
            vec![Ok(step(0.9, STOP, NONE, 0.95))],
        )
        .await;
        assert_eq!(none.expect("resolves").service, None);
        let (unsure, _) = run(
            DdfaMode::Acquire,
            &LIMITS,
            vec![Ok(step(0.9, STOP, "mds", 0.4))],
        )
        .await;
        assert_eq!(unsure.expect("resolves").service, None);
    }

    #[tokio::test]
    async fn once_fetches_first_hop_and_asks_target_only() {
        let (out, asked) = run(
            DdfaMode::Once,
            &LIMITS,
            vec![Ok(response(vec![(TARGET, choice("mds", 0.8))]))],
        )
        .await;
        let out = out.expect("resolves");
        assert_eq!(out.service.as_deref(), Some("mds"));
        assert_eq!(asked.len(), 1);
        assert_eq!(out.trail.len(), 5);
        let keys: Vec<&String> = asked[0]["questions"]
            .as_object()
            .expect("map")
            .keys()
            .collect();
        assert_eq!(keys, vec![TARGET]);
        assert_eq!(asked[0]["state"]["facts"].as_array().map(Vec::len), Some(5));
    }

    #[tokio::test]
    async fn jev_error_propagates() {
        let (out, _) = run(
            DdfaMode::Acquire,
            &LIMITS,
            vec![Err(TypeSafeError::Status {
                status: 500,
                body: String::new(),
            })],
        )
        .await;
        assert!(out.is_err());
    }
}
