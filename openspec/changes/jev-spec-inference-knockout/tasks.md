Implement with Code Writer subagents (`agents/writer/CLAUDE.md`); the orchestrator does not edit source. Read `design.md` (D1-D6) and both delta specs first. Start after `endpoint-route-prefixes` is done (shared files: `models`, pass 2).

Global constraints (every task):
- No `unwrap()` outside tests; no `#[allow(...)]`; `->` never the Unicode arrow; comment density like the surrounding code.
- Inference stage and spec application hold no language or framework names; knockout gates live next to the rule they gate.
- Static runs make no Jev call; default output unchanged.
- `cargo fmt` only on touched crates; `cargo clippy` adds no new warnings.
- Each task ends with `cargo test -p models -p python-extractor -p java-extractor -p go-extractor -p extractor-runtime -p clients -p synthesizer -p cli` green.
- Commit per task, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. `openspec/` stays uncommitted until the user decides.
- Scoring: Zenodo v2 `scoring/score.py` only; endpoint level `jev_triage_runs/score_endpoints.py`.
- Every Jev run costs credits: ask the user before 5.x runs, report expected question count first.

## 1. Knockout

- [ ] 1.1 Failing tests: `java-extractor` `resttemplate_knockout_identifies_nothing` (a RestTemplate `exchange` call file -> 0 raw REST calls with `java-resttemplate` in the set, 1 without); `python-extractor` `method_name_knockout_identifies_nothing` (same shape with `requests.get`). Verify: tests fail.
- [ ] 1.2 Implement D1: `KnockoutSet` in `models`, `identify(file, &KnockoutSet)` per extractor, rule-name constants, pass 2 threading, CLI `--knockout` (repeatable) with validation error listing known names, recorded in `run_metadata.json`. Verify: 1.1 passes; `cli` test `unknown_knockout_rule_rejected`; train-ticket static run without flag gives 236 requests.
- [ ] 1.3 Commit `feat: named extraction rule knockout`.

## 2. Spec model, candidates, application (no Jev)

- [ ] 2.1 Add `ClientSpec`, `ArgRef`, `MethodSource` (D4) and `RestCall.origin` (`Static` default, `Spec`; serde default, skipped when `Static`) to `models`. Verify: full test list green.
- [ ] 2.2 Failing tests in `extractor-runtime/src/pipeline/spec_inference/candidates.rs`: `project_function_not_candidate`, `claimed_call_not_candidate`, `url_literal_arg_is_candidate`, `url_named_identifier_is_candidate`, `constant_with_scheme_is_candidate`, `symbol_key_prefers_receiver_type`, `candidates_grouped_by_symbol_in_stable_order`. Implement D2. Verify: pass.
- [ ] 2.3 Failing tests in `.../spec_inference/apply.rs`: `spec_with_fixed_method_builds_restcall`, `spec_with_method_arg_reads_argument`, `keyword_url_arg`, `rejected_spec_builds_nothing`, `origin_is_spec`. Implement D5. Verify: pass.
- [ ] 2.4 Cache read/write with sorted keys; test `cache_roundtrip_is_byte_stable`. Commit `feat(extractor-runtime): client spec candidates, application, cache`.

## 3. Jev questions and stage

- [ ] 3.1 Decide fact loop: read `extractor-runtime/src/pipeline/pass3/llm_enhance/ddfa.rs`; reuse its loop if facts can be keyed by symbol, else `Once` only. Record the choice in `jev_triage_runs/spec-knockout-v1/results.md`. Verify: note written.
- [ ] 3.2 `clients/src/typesafe/question/spec.rs`: `is_http()`, `url_arg(opts)`, `method(opts)` per D3; tests `url_arg_adds_none`, `method_lists_fixed_and_arg_options`, `too_many_options_errors`. Verify: `cargo test -p clients` green.
- [ ] 3.3 `spec_inference::infer(ir, cache, min_p, ask) -> (Vec<ClientSpec>, SpecStats)` with scripted-`ask` tests: `accepts_above_threshold`, `rejects_below_threshold`, `none_url_rejects`, `cached_symbol_not_asked`, `ask_error_skips_symbol`, `one_request_per_symbol`. Verify: pass.
- [ ] 3.4 Wire into `cli/src/lib.rs` between `build_project_ir` and `evaluate`; flags `--jev-specs` (requires `--typesafe`), `--spec-cache`, env `JEV_SPEC_MIN` (default 0.7); print `[spec-jev] <symbol> <accepted> <p> <tokens>` per symbol and a total row. Verify: `--help` lists flags; static run unchanged (236 requests TT). Commit `feat: Jev client spec inference (--jev-specs)`.

## 4. Dry run (free)

- [ ] 4.1 Add `--spec-dry-run`: list candidate symbols, call-site counts, and estimated tokens (symbols x 600) without asking. Run on each arm's KO setting. Verify: table in `results.md` with candidate symbols per corpus; show to the user before any Jev run.

## 5. Experiment (costs credits, user approval per step)

- [ ] 5.1 static and KO arms (free) on TT, polyglot, empaia into `jev_triage_runs/spec-knockout-v1/`; score. Verify: rows `static`, `KO` present; KO recall < static recall (else the knockout is ineffective, stop and report).
- [ ] 5.2 KO+spec arm; then KO+spec cached rerun; score; set diff of requests between the two. Verify: rows present; cached run made 0 Jev calls and set diff 0.
- [ ] 5.3 natural+spec on TT and polyglot: is voucher -> order / order-other recovered via `urllib`? Verify: row present, voucher edges listed.
- [ ] 5.4 Leakage control: polyglot with the client import aliased (`import requests as hx`, or a renamed copy of the corpus under the scratchpad) -> KO+spec recovery compared to unaliased. Verify: row present.
- [ ] 5.5 Uncached variance: 3 uncached KO+spec runs on polyglot; report spec agreement and recall spread. Verify: row present.
- [ ] 5.6 Report to user: recovery (service + endpoint), precision, Jev calls / tokens vs estimated per-call-site cost, determinism, leakage gap, FPs by symbol, recommendation (keep opt-in / default / drop; next arm: endpoint route specs).
