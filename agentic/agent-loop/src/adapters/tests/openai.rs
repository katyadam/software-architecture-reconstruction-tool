use agentic_models::{Fact, FactId};
use serde_json::{Value, json};

use super::{answers, options};
use crate::adapters::openai::NONE;
use crate::adapters::openai::reply::{reply_from, usage};
use crate::adapters::openai::request::{picks, render, request};
use crate::adapters::{AnswerOption, Answers};
use async_openai::Client;
use async_openai::config::OpenAIConfig;

use crate::adapters::OpenAiModel;
use crate::protocol::{Answer, Model, Request, Step};

fn reply(content: &str, prompt_tokens: Option<u64>) -> Value {
    let mut v = json!({
        "choices": [{ "message": { "content": content } }],
    });
    if let Some(p) = prompt_tokens {
        v["usage"] = json!({ "prompt_tokens": p, "completion_tokens": 7 });
    }
    v
}

fn step_for(content: &str) -> Result<Step<String>, String> {
    reply_from(&reply(content, Some(10)), &answers(), None).map(|r| r.step)
}

#[test]
fn picks_hold_offered_ids_labels_and_none() {
    let opts = options(&["open:a.py", "define:x"]);
    assert_eq!(
        picks(&opts, &answers()),
        vec![
            "call:open:a.py",
            "call:define:x",
            "answer:orders",
            "answer:payments",
            NONE
        ]
    );
}

#[test]
fn no_options_only_answer_picks() {
    assert_eq!(
        picks(&[], &answers()),
        vec!["answer:orders", "answer:payments", NONE]
    );
}

#[test]
fn request_carries_schema_and_zero_temperature() {
    let opts = options(&["open:a.py"]);
    let req = Request {
        task: "Where does this call go?",
        facts: &[],
        options: &opts,
    };
    let built = request(&req, &answers(), "deepseek-v4-flash").expect("builds");
    let v = serde_json::to_value(&built).expect("serializable");
    assert_eq!(v["model"], "deepseek-v4-flash");
    assert_eq!(v["temperature"], 0.0);
    let schema = &v["response_format"]["json_schema"]["schema"];
    assert_eq!(schema["properties"]["pick"]["enum"][0], "call:open:a.py");
}

#[test]
fn render_lists_task_facts_tools_answers() {
    let facts = vec![Fact {
        id: FactId("f1".into()),
        tool: "define".into(),
        args: "BASE".into(),
        text: "BASE = settings.mds_url".into(),
    }];
    let opts = options(&["open:a.py"]);
    let req = Request {
        task: "T",
        facts: &facts,
        options: &opts,
    };
    let text = render(&req, &answers());
    for part in [
        "Task:\nT",
        "f1 [define BASE]\nBASE = settings.mds_url",
        "- call:open:a.py: open open:a.py",
        "- answer:payments: service payments",
        "- none:",
    ] {
        assert!(text.contains(part), "missing {part:?} in {text}");
    }
}

#[test]
fn pick_a_tool() {
    let content = r#"{"pick": "call:open:a.py#settings", "note": "need the base URL"}"#;
    assert_eq!(
        step_for(content),
        Ok(Step::Call("open:a.py#settings".into()))
    );
}

#[test]
fn pick_an_answer_keeps_note() {
    let content = r#"{"pick": "answer:payments", "note": "settings.mds_url points to payments"}"#;
    let expected = Answer {
        value: "PAYMENTS".to_string(),
        confidence: None,
        note: Some("settings.mds_url points to payments".into()),
    };
    assert_eq!(step_for(content), Ok(Step::Final(expected)));
}

#[test]
fn pick_none_gives_none_value() {
    let Ok(Step::Final(a)) = step_for(r#"{"pick": "none", "note": "external"}"#) else {
        panic!("answers")
    };
    assert_eq!(a.value, "");
}

#[test]
fn tool_id_and_label_never_collide() {
    let answers = Answers {
        options: vec![AnswerOption {
            label: "payments".into(),
            description: "service payments".into(),
            value: "PAYMENTS".to_string(),
        }],
        none: String::new(),
    };
    let step = |pick: &str| {
        let content = format!(r#"{{"pick": "{pick}", "note": ""}}"#);
        reply_from(&reply(&content, Some(1)), &answers, None).map(|r| r.step)
    };
    assert_eq!(step("call:payments"), Ok(Step::Call("payments".into())));
    assert!(matches!(step("answer:payments"), Ok(Step::Final(_))));
}

#[test]
fn unoffered_answer_is_error() {
    assert!(step_for(r#"{"pick": "answer:inventory", "note": ""}"#).is_err());
}

/// The loop, not the adapter, rejects unoffered ids or ends a spent budget.
#[test]
fn any_call_goes_to_the_loop() {
    assert_eq!(
        step_for(r#"{"pick": "call:open:z.py", "note": ""}"#),
        Ok(Step::Call("open:z.py".into()))
    );
}

#[test]
fn unparsable_reply_is_error() {
    assert!(step_for("The service is payments.").is_err());
}

#[test]
fn usage_reported() {
    let r = reply_from(
        &reply(r#"{"pick": "none", "note": ""}"#, Some(300)),
        &answers(),
        None,
    )
    .expect("parses");
    assert_eq!((r.input_tokens, r.output_tokens), (300, 7));
}

#[test]
fn silent_truncation_caught() {
    let err = usage(&reply("", Some(8192)), Some(8192)).expect_err("overflow");
    assert!(err.contains("8192"));
}

#[test]
fn within_context_succeeds() {
    assert_eq!(usage(&reply("", Some(3000)), Some(8192)), Ok((3000, 7)));
}

#[test]
fn missing_usage_with_context_size_is_error() {
    assert!(usage(&reply("", None), Some(8192)).is_err());
}

#[test]
fn no_check_configured() {
    assert_eq!(usage(&reply("", None), None), Ok((0, 0)));
}

#[test]
fn model_id_follows_answers() {
    let client = Client::with_config(OpenAIConfig::new());
    let id = |a: Answers<String>| OpenAiModel::new(&client, "m", a, None).id().to_string();
    let base = id(answers());
    assert!(base.starts_with("m#"));
    let mut changed = answers();
    changed.options[0].description = "the order service".into();
    assert_ne!(id(changed), base);
    assert_eq!(id(answers()), base);
}
