use std::collections::HashMap;

use clients::typesafe::JEV_MODEL;
use clients::typesafe::question::{MAX_CHOICE_OPTIONS, NoulCriteria, Question};
use clients::typesafe::response::{Answer as JevAnswer, SystemOneResponse, Usage};

use super::{answers, options};
use crate::adapters::jev::model_id;
use crate::adapters::jev::questions::questions;
use crate::adapters::jev::reply::step_from;
use crate::adapters::jev::{ANSWER, ENOUGH, NEXT, NONE, STOP};
use crate::adapters::{JevLimits, JevWording};
use crate::protocol::{Answer, Step};

fn wording() -> JevWording {
    JevWording {
        answer: "Which service?".into(),
        enough: "Enough facts?".into(),
        enough_criteria: NoulCriteria {
            yes: "yes".into(),
            no: "no".into(),
        },
        next: "Which fact next?".into(),
    }
}

fn choice(pick: &str, p: f64) -> JevAnswer {
    JevAnswer::Choice {
        choice: pick.into(),
        probabilities: HashMap::from([(pick.to_string(), p)]),
        confidence: p,
    }
}

fn response(answers: Vec<(&str, JevAnswer)>) -> SystemOneResponse {
    SystemOneResponse {
        model: "jev-1.13.0".into(),
        answers: answers
            .into_iter()
            .map(|(k, a)| (k.to_string(), a))
            .collect(),
        usage: Usage {
            input_tokens: 392,
            output_tokens: 65,
        },
    }
}

/// `enough`, `next` and `answer` in one reply.
fn step(enough: f64, next: &str, pick: &str, p: f64) -> Result<Step<String>, String> {
    let r = response(vec![
        (ENOUGH, JevAnswer::Noul { noul: enough }),
        (NEXT, choice(next, 0.9)),
        (ANSWER, choice(pick, p)),
    ]);
    step_from(&r, &answers(), JevLimits::default(), true)
}

fn keys(q: &std::collections::BTreeMap<String, Question>) -> Vec<&str> {
    q.keys().map(String::as_str).collect()
}

#[test]
fn no_options_asks_answer_only() {
    let q = questions(&wording(), &answers(), &[]).expect("fits");
    assert_eq!(keys(&q), vec![ANSWER]);
    let Question::Choice { criteria, .. } = &q[ANSWER] else {
        panic!("answer is a choice")
    };
    assert_eq!(
        criteria.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![NONE, "orders", "payments"]
    );
}

#[test]
fn options_ask_all_three() {
    let q = questions(&wording(), &answers(), &options(&["open:a.py"])).expect("fits");
    assert_eq!(keys(&q), vec![ANSWER, ENOUGH, NEXT]);
    let Question::Choice { criteria, .. } = &q[NEXT] else {
        panic!("next is a choice")
    };
    assert!(criteria.contains_key(STOP) && criteria.contains_key("open:a.py"));
}

#[test]
fn too_many_options_keep_the_best() {
    let ids: Vec<String> = (0..300).map(|i| format!("open:{i:03}.py")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let q = questions(&wording(), &answers(), &options(&refs)).expect("truncates");
    let Question::Choice { criteria, .. } = &q[NEXT] else {
        panic!("next is a choice")
    };
    assert_eq!(criteria.len(), MAX_CHOICE_OPTIONS);
    assert!(criteria.contains_key("open:000.py") && criteria.contains_key("open:253.py"));
    assert!(!criteria.contains_key("open:254.py") && criteria.contains_key(STOP));
}

#[test]
fn reserved_or_duplicate_ids_are_errors() {
    let mut a = answers();
    a.options[0].label = NONE.into();
    assert!(questions(&wording(), &a, &[]).is_err());
    let mut a = answers();
    a.options[1].label = "orders".into();
    assert!(questions(&wording(), &a, &[]).is_err());
    assert!(questions(&wording(), &answers(), &options(&[STOP])).is_err());
    assert!(questions(&wording(), &answers(), &options(&["x", "x"])).is_err());
}

#[test]
fn model_id_follows_settings() {
    let id = |w: &JevWording, l: JevLimits| model_id(w, l, &answers());
    let base = id(&wording(), JevLimits::default());
    assert!(base.starts_with(JEV_MODEL));
    let mut w = wording();
    w.answer = "Which endpoint?".into();
    assert_ne!(id(&w, JevLimits::default()), base);
    let l = JevLimits {
        enough: 0.8,
        ..JevLimits::default()
    };
    assert_ne!(id(&wording(), l), base);
    assert_eq!(id(&wording(), JevLimits::default()), base);
}

#[test]
fn not_enough_fetches() {
    assert_eq!(
        step(0.4, "open:a.py#settings", "payments", 0.8),
        Ok(Step::Call("open:a.py#settings".into()))
    );
}

#[test]
fn enough_answers_with_confidence() {
    let expected = Answer {
        value: "PAYMENTS".to_string(),
        confidence: Some(0.8),
        note: None,
    };
    assert_eq!(
        step(0.9, "open:a.py", "payments", 0.8),
        Ok(Step::Final(expected))
    );
}

#[test]
fn unsure_answer_abstains() {
    let Ok(Step::Final(a)) = step(0.9, "open:a.py", "payments", 0.3) else {
        panic!("answers")
    };
    assert_eq!((a.value.as_str(), a.confidence), ("", None));
    assert_eq!(a.note.as_deref(), Some("below 0.5: payments 0.3"));
}

#[test]
fn stop_answers() {
    assert!(matches!(
        step(0.1, STOP, "orders", 0.9),
        Ok(Step::Final(Answer { value, .. })) if value == "ORDERS"
    ));
}

#[test]
fn no_options_answers_whatever_next_says() {
    let r = response(vec![(ANSWER, choice("orders", 0.9))]);
    let s = step_from(&r, &answers(), JevLimits::default(), false);
    assert!(matches!(s, Ok(Step::Final(_))));
}

#[test]
fn missing_next_answers() {
    let r = response(vec![
        (ENOUGH, JevAnswer::Noul { noul: 0.1 }),
        (ANSWER, choice("orders", 0.9)),
    ]);
    let s = step_from(&r, &answers(), JevLimits::default(), true);
    assert!(matches!(s, Ok(Step::Final(_))));
}

#[test]
fn answer_outside_set_is_error() {
    assert!(step(0.9, "open:a.py", "inventory", 0.9).is_err());
}

#[test]
fn none_pick_gives_none_value() {
    let Ok(Step::Final(a)) = step(0.9, "open:a.py", NONE, 0.9) else {
        panic!("answers")
    };
    assert_eq!(a.value, "");
}
