# Jev triage — generalization check on train-ticket

Self-contained brief for a cold session. Goal: does the V5 Jev residual triage
(P = R = 1.00 on empaia) hold on a second corpus, mostly Java?

- **Branch:** `sage-rework` (V5 = commit `07538f8`, default `JEV_THRESHOLD`
  0.8 since `c32b68b`).
- **Scope:** measurement only. Do not change the question, the state, or the
  threshold logic. If results are bad, record why. Do not tune on train-ticket
  in this task.

---

## 1. Background (what exists)

- REST calls whose `target_uri` is not a URL after symbolic evaluation are
  **residuals**. Many are not HTTP at all: DB clients, dict `.get`, docker SDK.
- Triage sends each residual to TypeSafe Jev (`jev-1.13.0`, System One API).
  The request has one Noul question, `is_http`. The verdict is an edge candidate
  (`NeedsResolution`) iff `is_http >= JEV_THRESHOLD`, otherwise `NonEdge`.
- Code:
  - `extractor-runtime/src/pipeline/pass3/llm_enhance/residual_edge_filter.rs`:
    triage, state building, log rows.
  - `clients/src/typesafe/question/residual.rs`: `is_http` question text.
  - `clients/src/typesafe/state.rs`: `ResidualCallState` (call, receiver,
    residual, operand_bindings, enclosing, snippet, imports, file).
- State in V5 includes `snippet`, the source of the innermost enclosing function
  (`ParsedCallStatement.function_span`, read from disk).
- empaia results: `jev_triage_runs/summary.md` and
  `jev_triage_runs/v5-function-snippet/results.md`.
  - V5 on empaia: 128 residuals, 20 HTTP. P/R 1.00/1.00 at t = 0.9 and 0.8.
  - Every HTTP residual scored >= 0.96; every non-HTTP <= 0.72.
- **Untested so far:** Java. Everything above is Python (empaia).

## 2. Why train-ticket

- `../train-ticket`, 47 configured services, mostly Java (Spring
  `RestTemplate`), plus a few non-Java services.
- An earlier run (2026-07-11, `docs/sage_rework_results.md` S3.2) found
  ~24 residuals, ~22 of them looking cross-service. So the class balance is
  the **opposite** of empaia (mostly positives). Recall and false negatives
  matter most here.
- The residual count may have changed since. Record the real number.

## 3. Risks to check explicitly

Java-specific code paths that the empaia runs never exercised:

1. **Snippet presence.** Does the Java extractor set
   `ParsedCallStatement.function_span`? Count states with `"snippet"` in the
   log. empaia had 128/128. If Java has 0, the run is effectively V4, not V5.
   Say so in the results.
2. **`source_call` match.** It matches by `enclosing_function_hash` +
   `arguments`. If it misses, `call` falls back to `GET(...)` (HTTP method
   instead of callee) and `receiver` is absent. Count states whose `call`
   starts with an HTTP method name.
3. **`strip_args`.** Java `function_name` carries args
   (`restTemplate.exchange(url, ...)`). Check that `call` and `receiver.expr`
   look right on a few rows.
4. **`receiver.datatype`.** Is `invoked_on` populated for Java
   (e.g. `RestTemplate`)?
5. **`imports`.** Present and readable for Java files?

Report each as a count or yes/no in the results.

## 4. Run

Prerequisites:

- `TYPESAFE_API_KEY` set (the CLI loads `.env` via `dotenvy`).
- `../train-ticket` exists. (`../train-ticket-polyglot`, used by
  `local-train-ticket-polyglot-config.json`, is **not** checked out. Skip it
  unless it appears.)
- Ollama/Sage not needed: point `--llm-url` at a dead port so resolution fails
  instantly and adds no LLM time (same trick as the empaia runs).

```bash
cargo build --release -p cli
mkdir -p jev_triage_runs/tt-v5-function-snippet
for t in 0.9 0.8 0.7; do
  JEV_THRESHOLD=$t \
  JEV_LOG=jev_triage_runs/tt-v5-function-snippet/is_http-$t.log \
  target/release/cli -p ../train-ticket \
    -c config/configurations/local-train-ticket-config.json \
    -o /tmp/tt-jev-$t \
    --scrape --llm --typesafe --llm-url http://127.0.0.1:9/v1
done
```

- No `-f`: there is no train-ticket constants file.
- Three thresholds = three runs, so we also see run-to-run drift (~±0.02 on
  empaia).
- Cost is negligible: empaia was ~$0.0025 per run.
- Check for `JevError` rows. If there are any, note the status/body. Do not
  retry silently.

## 5. Ground truth (manual labelling)

Label every distinct residual in the log by reading the train-ticket source.

- **Positive (HTTP):** the call sends an HTTP request to any host. This
  includes configured services, unconfigured services, and external hosts.
  Same rule as empaia (`jev_triage_runs/v1-is_http/results.md`, section
  "Ground truth").
- **Negative:** anything else: DB/JPA/repository, Map/collection `.get`,
  cache, message broker, etc.
- Key each label by `(file suffix, call prefix)`, as in `score.py`.
- Put the table in the results file: label, call, file, and a one-line
  reason for any non-obvious label.
- `../train-ticket/benchmark-ground-truth/rest-edges.yaml` is the
  service-edge oracle. It helps confirm which calls are cross-service, but it
  is **not** the triage label. Triage asks "is this HTTP?", not "is this a
  business edge?".

## 6. Scoring

`jev_triage_runs/score.py` hardcodes empaia's `GROUND_TRUTH` list.

Minimal change: make the ground truth selectable per corpus. For example,
move the empaia list into a dict keyed by corpus, add a `train-ticket` entry,
and pick it with a second CLI argument (default `empaia`, so old commands
still work). Do not restructure anything else. Re-run it on
`jev_triage_runs/v5-function-snippet` afterwards and confirm the empaia
numbers are unchanged.

```bash
python3 jev_triage_runs/score.py jev_triage_runs/tt-v5-function-snippet train-ticket
```

## 7. Outputs

1. `jev_triage_runs/tt-v5-function-snippet/results.md`, in the same shape as
   `v5-function-snippet/results.md`:
   - Version/commit, command, date, and residual count (HTTP vs non-HTTP).
   - The results table at t = 0.9 / 0.8 / 0.7: TP FP FN TN, P R F1, Jev
     errors, wall time, latency (mean/p50/p95/max), and input tokens.
   - **Separation:** the lowest HTTP score and the highest non-HTTP score, and
     whether any t separates them in all runs.
   - Every misclassified residual, with its score and a one-line cause
     (which state field misled or was missing).
   - Section 3 risk checks, with counts.
   - Ground-truth table.
   - A "Reading" section with the verdict (see section 8).
2. One row added to `jev_triage_runs/summary.md`, marked as train-ticket.
3. A dated entry in `docs/sage_rework_results.md`. That file is the durable
   log, and results not recorded there are lost.
4. Commit on `sage-rework`. Message style:
   `Measure V5 Jev triage on train-ticket`.

## 8. Verdict criteria

- **Generalizes:** P >= 0.95 and R >= 0.95 at the default t = 0.8, and some
  t separates the classes in all three runs. Next: the end-to-end re-score
  with the oracle, then Jev `Choice` for the target service.
- **Partial:** the scores separate, but at a different t than empaia. Record
  the per-corpus separating ranges. A threshold valid for both corpora is the
  intersection of the ranges. Report it if non-empty.
- **Fails:** FNs on Java HTTP calls. Diagnose from the state: is a section 3
  field missing, or is the question wording at fault? Name the fix candidates
  (e.g. Java `function_span`, a Spring-specific framework name in the
  criteria). **Do not apply them in this task.** A fix becomes the next
  version, measured on both corpora.

## 9. Constraints

- Follow the repo `CLAUDE.md` (agentic pipeline for code changes; the
  `score.py` tweak is small enough to do inline).
- No `unwrap()` in service code. Do not commit `.env`.
- Keep reports concise. Use `->`, never the arrow glyph.
