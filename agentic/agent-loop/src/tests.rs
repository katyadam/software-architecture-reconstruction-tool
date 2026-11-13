use std::collections::{BTreeSet, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use agentic_models::{Fact, FactId};

use super::*;

/// Replays a script of replies; records the option ids offered per call.
struct Scripted {
    id: String,
    script: VecDeque<Result<Reply<String>, String>>,
    offered: Vec<Vec<String>>,
}

impl Scripted {
    fn new(script: Vec<Result<Reply<String>, String>>) -> Self {
        Self {
            id: "fake-1".into(),
            script: script.into(),
            offered: vec![],
        }
    }
}

impl Model<String> for Scripted {
    fn id(&self) -> &str {
        &self.id
    }

    async fn step(&mut self, req: &Request<'_>) -> Result<Reply<String>, String> {
        self.offered
            .push(req.options.iter().map(|o| o.id.clone()).collect());
        self.script
            .pop_front()
            .unwrap_or_else(|| Err("script empty".into()))
    }
}

/// `open_file` on each listed file; output names the file.
struct Files(Vec<&'static str>);

impl Tools for Files {
    fn available(&self, taken: &BTreeSet<String>) -> Vec<ToolOption> {
        self.0
            .iter()
            .map(|f| ToolOption {
                id: format!("open:{f}"),
                tool: "open_file".into(),
                args: f.to_string(),
                description: format!("Open {f}"),
            })
            .filter(|o| !taken.contains(&o.id))
            .collect()
    }

    fn run(&self, option: &ToolOption) -> String {
        format!("contents of {}", option.args)
    }
}

fn call(id: &str) -> Result<Reply<String>, String> {
    Ok(Reply {
        step: Step::Call(id.into()),
        input_tokens: 100,
        output_tokens: 5,
    })
}

fn answer(cites: &[&str]) -> Result<Reply<String>, String> {
    Ok(Reply {
        step: Step::Final {
            answer: "svc-b".into(),
            cites: cites.iter().map(|c| FactId(c.to_string())).collect(),
        },
        input_tokens: 150,
        output_tokens: 10,
    })
}

fn key(subject: &str) -> SessionKey {
    SessionKey {
        stage: "matcher".into(),
        subject: subject.into(),
    }
}

/// Fresh log file per test name.
fn temp_log(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("agent-loop-{}-{name}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn files() -> Files {
    Files(vec!["a.py", "b.py", "c.py"])
}

async fn run(name: &str, model: &mut Scripted, budget: usize) -> Session<String> {
    let log = Log::open(&temp_log(name)).expect("open log");
    run_session(&key("P"), "task", model, &files(), budget, &log)
        .await
        .expect("session")
}

fn answered(s: &Session<String>) -> bool {
    matches!(s.outcome, Outcome::Answer { .. })
}

fn model_error(s: &Session<String>) -> bool {
    matches!(s.outcome, Outcome::ModelError(_))
}

#[tokio::test]
async fn tool_call_then_answer() {
    let mut m = Scripted::new(vec![call("open:a.py"), answer(&["f1"])]);
    let s = run("tool-then-answer", &mut m, 6).await;
    assert!(answered(&s));
    assert_eq!((s.calls, s.facts.len()), (2, 1));
}

#[tokio::test]
async fn answer_at_once() {
    let mut m = Scripted::new(vec![answer(&[])]);
    let s = run("answer-at-once", &mut m, 6).await;
    assert!(answered(&s));
    assert_eq!((s.calls, s.facts.len()), (1, 0));
}

#[tokio::test]
async fn facts_numbered_in_order() {
    let mut m = Scripted::new(vec![call("open:b.py"), call("open:a.py"), answer(&["f2"])]);
    let s = run("facts-numbered", &mut m, 6).await;
    let expected = |id: &str, file: &str| Fact {
        id: FactId(id.into()),
        tool: "open_file".into(),
        args: file.into(),
        text: format!("contents of {file}"),
    };
    assert_eq!(
        s.facts,
        vec![expected("f1", "b.py"), expected("f2", "a.py")]
    );
}

#[tokio::test]
async fn numbering_restarts_per_session() {
    let log = Log::open(&temp_log("numbering-restarts")).expect("open log");
    for subject in ["P", "Q"] {
        let mut m = Scripted::new(vec![call("open:a.py"), answer(&["f1"])]);
        let s = run_session(&key(subject), "task", &mut m, &files(), 6, &log)
            .await
            .expect("session");
        assert_eq!(s.facts[0].id, FactId("f1".into()));
    }
}

#[tokio::test]
async fn unknown_citation_is_model_error() {
    let mut m = Scripted::new(vec![call("open:a.py"), answer(&["f2"])]);
    assert!(model_error(&run("unknown-cite", &mut m, 6).await));
}

#[tokio::test]
async fn same_tool_different_files() {
    let mut m = Scripted::new(vec![
        call("open:a.py"),
        call("open:b.py"),
        answer(&["f1", "f2"]),
    ]);
    let s = run("same-tool", &mut m, 6).await;
    assert!(answered(&s));
    let args: Vec<_> = s
        .facts
        .iter()
        .map(|f| (f.tool.as_str(), f.args.as_str()))
        .collect();
    assert_eq!(args, [("open_file", "a.py"), ("open_file", "b.py")]);
}

#[tokio::test]
async fn taken_option_not_offered_again() {
    let mut m = Scripted::new(vec![call("open:a.py"), answer(&[])]);
    run("taken-not-offered", &mut m, 6).await;
    assert!(m.offered[0].contains(&"open:a.py".to_string()));
    assert!(!m.offered[1].contains(&"open:a.py".to_string()));
}

#[tokio::test]
async fn call_to_unknown_option_is_model_error() {
    let mut m = Scripted::new(vec![call("open:z.py")]);
    assert!(model_error(&run("unknown-option", &mut m, 6).await));
}

#[tokio::test]
async fn budget_reached_forces_tool_free_call() {
    let mut m = Scripted::new(vec![call("open:a.py"), call("open:b.py"), answer(&["f2"])]);
    let s = run("budget-reached", &mut m, 2).await;
    assert!(answered(&s));
    assert_eq!((s.calls, s.facts.len()), (3, 2));
    assert!(m.offered[2].is_empty());
}

#[tokio::test]
async fn budget_exhausted_when_last_call_is_a_tool() {
    let mut m = Scripted::new(vec![call("open:a.py"), call("open:b.py")]);
    let s = run("budget-exhausted", &mut m, 1).await;
    assert_eq!(s.outcome, Outcome::BudgetExhausted);
    assert_eq!(s.facts.len(), 1);
}

#[tokio::test]
async fn budget_zero_is_one_tool_free_call() {
    let mut m = Scripted::new(vec![answer(&[])]);
    let s = run("budget-zero", &mut m, 0).await;
    assert_eq!((s.calls, s.facts.len()), (1, 0));
    assert_eq!(m.offered, vec![Vec::<String>::new()]);
}

#[tokio::test]
async fn provider_failure_is_model_error() {
    let mut m = Scripted::new(vec![call("open:a.py"), Err("HTTP 503".into())]);
    let s = run("provider-fails", &mut m, 6).await;
    assert_eq!(s.outcome, Outcome::ModelError("HTTP 503".into()));
}

#[tokio::test]
async fn tokens_summed() {
    let mut m = Scripted::new(vec![call("open:a.py"), answer(&[])]);
    let s = run("tokens", &mut m, 6).await;
    assert_eq!((s.calls, s.input_tokens, s.output_tokens), (2, 250, 15));
}

#[test]
fn prompt_hash_follows_task_and_model() {
    let base = prompt_hash("m", 6, "task");
    assert_eq!(base, prompt_hash("m", 6, "task"));
    assert_ne!(base, prompt_hash("m", 6, "task v2"));
    assert_ne!(base, prompt_hash("m2", 6, "task"));
    assert_ne!(base, prompt_hash("m", 3, "task"));
    // Parts cannot run into each other.
    assert_ne!(prompt_hash("m", 6, "x\ny"), prompt_hash("m\nx", 6, "y"));
}

#[tokio::test]
async fn answer_logged_as_one_line() {
    let path = temp_log("answer-logged");
    let log = Log::open(&path).expect("open log");
    let mut m = Scripted::new(vec![call("open:a.py"), answer(&["f1"])]);
    run_session(&key("P"), "task", &mut m, &files(), 6, &log)
        .await
        .expect("session");
    let text = std::fs::read_to_string(&path).expect("read log");
    let line: serde_json::Value = serde_json::from_str(text.trim_end()).expect("one json line");
    assert_eq!(text.lines().count(), 1);
    assert_eq!(line["stage"], "matcher");
    assert_eq!(line["subject"], "P");
    assert_eq!(line["prompt_hash"], prompt_hash("fake-1", 6, "task"));
    assert_eq!(line["facts"][0]["id"], "f1");
    assert_eq!(line["outcome"]["Answer"]["answer"], "svc-b");
    assert_eq!(line["input_tokens"], 250);
}

#[tokio::test]
async fn model_error_not_logged() {
    let path = temp_log("error-not-logged");
    let log = Log::open(&path).expect("open log");
    let mut m = Scripted::new(vec![Err("timeout".into())]);
    run_session(&key("P"), "task", &mut m, &files(), 6, &log)
        .await
        .expect("session");
    assert_eq!(std::fs::read_to_string(&path).expect("read log"), "");
}

/// Runs subject P on a fresh `Log` over `path`, like a restarted process.
async fn rerun(
    path: &Path,
    task: &str,
    script: Vec<Result<Reply<String>, String>>,
) -> (Session<String>, usize) {
    let log = Log::open(path).expect("open log");
    let mut m = Scripted::new(script);
    let s = run_session(&key("P"), task, &mut m, &files(), 6, &log)
        .await
        .expect("session");
    (s, m.offered.len())
}

#[tokio::test]
async fn rerun_after_crash_replays() {
    let path = temp_log("rerun");
    let (first, _) = rerun(&path, "task", vec![call("open:a.py"), answer(&["f1"])]).await;
    let (second, calls) = rerun(&path, "task", vec![]).await;
    assert_eq!(calls, 0);
    assert!(second.replayed);
    assert_eq!((second.outcome, second.facts), (first.outcome, first.facts));
}

#[tokio::test]
async fn changed_prompt_reruns() {
    let path = temp_log("changed-prompt");
    rerun(&path, "task", vec![answer(&[])]).await;
    let (s, calls) = rerun(&path, "task v2", vec![answer(&[])]).await;
    assert_eq!(calls, 1);
    assert!(!s.replayed);
}

#[tokio::test]
async fn errored_point_retried() {
    let path = temp_log("errored-retried");
    rerun(&path, "task", vec![Err("timeout".into())]).await;
    let (s, calls) = rerun(&path, "task", vec![answer(&[])]).await;
    assert_eq!(calls, 1);
    assert!(answered(&s));
}

#[tokio::test]
async fn truncated_last_line_keeps_earlier_records() {
    let path = temp_log("truncated");
    rerun(&path, "task", vec![answer(&[])]).await;
    // Crash mid-write: half a record, no newline.
    let mut file = OpenOptions::new().append(true).open(&path).expect("open");
    file.write_all(br#"{"stage":"matcher","subj"#)
        .expect("write");
    drop(file);

    let (s, calls) = rerun(&path, "task", vec![]).await;
    assert_eq!(calls, 0);
    assert!(s.replayed);

    // The next record starts on its own line and loads.
    let log = Log::open(&path).expect("open log");
    let mut m = Scripted::new(vec![answer(&[])]);
    run_session(
        &SessionKey {
            stage: "verifier".into(),
            subject: "L".into(),
        },
        "task",
        &mut m,
        &files(),
        0,
        &log,
    )
    .await
    .expect("session");
    let log = Log::open(&path).expect("reopen log");
    let mut m = Scripted::new(vec![]);
    let s = run_session::<String, _, _>(
        &SessionKey {
            stage: "verifier".into(),
            subject: "L".into(),
        },
        "task",
        &mut m,
        &files(),
        0,
        &log,
    )
    .await
    .expect("session");
    assert!(s.replayed);
}

#[tokio::test]
async fn session_keyed_by_stage_and_subject() {
    let path = temp_log("keyed");
    rerun(&path, "task", vec![answer(&[])]).await;
    let log = Log::open(&path).expect("open log");
    let point = "svc-a/client.py#a1b2#rest_call#{base}/cases#10-40";
    let link = "svc-a/client.py#a1b2#rest_call#{base}/cases#10-40->svc-b";
    for key in [
        SessionKey {
            stage: "verifier".into(),
            subject: "P".into(),
        },
        SessionKey {
            stage: "matcher".into(),
            subject: point.into(),
        },
        SessionKey {
            stage: "verifier".into(),
            subject: link.into(),
        },
    ] {
        let mut m = Scripted::new(vec![answer(&[])]);
        let s = run_session(&key, "task", &mut m, &files(), 6, &log)
            .await
            .expect("session");
        assert!(!s.replayed, "{key:?} must not hit matcher/P");
    }
}

#[tokio::test]
async fn call_when_tools_run_out_is_budget_exhausted() {
    let mut m = Scripted::new(vec![
        call("open:a.py"),
        call("open:b.py"),
        call("open:c.py"),
        call("open:a.py"),
    ]);
    let s = run("tools-run-out", &mut m, 6).await;
    assert_eq!(s.outcome, Outcome::BudgetExhausted);
    assert!(m.offered[3].is_empty());
}

#[tokio::test]
async fn changed_budget_reruns() {
    let path = temp_log("changed-budget");
    let log = Log::open(&path).expect("open log");
    for budget in [0, 3] {
        let mut m = Scripted::new(vec![answer(&[])]);
        let s = run_session(&key("L"), "task", &mut m, &files(), budget, &log)
            .await
            .expect("session");
        assert!(!s.replayed, "budget {budget} must call the model");
    }
}

#[tokio::test]
async fn invalid_utf8_line_skipped_not_fatal() {
    let path = temp_log("bad-utf8");
    rerun(&path, "task", vec![answer(&[])]).await;
    let mut file = OpenOptions::new().append(true).open(&path).expect("open");
    file.write_all(b"{\"stage\":\"m\xc3").expect("write");
    drop(file);
    let (s, calls) = rerun(&path, "task", vec![]).await;
    assert_eq!(calls, 0);
    assert!(s.replayed);
}
