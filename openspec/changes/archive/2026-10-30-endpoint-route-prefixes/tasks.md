Implement with Code Writer subagents (`agents/writer/CLAUDE.md`); the orchestrator does not edit source. Read `design.md` (D1-D6) and `specs/endpoint-route-prefixes/spec.md` first.

Global constraints (every task):
- No `unwrap()` outside tests; no `#[allow(...)]`; `->` never the Unicode arrow; comment density like the surrounding code.
- No language or framework names inside the resolver (`route_prefixes.rs`); framework knowledge lives only in adapters.
- `cargo fmt` only on touched crates (revert unrelated reformatting, e.g. `sage/src/classifier/mod.rs`); `cargo clippy` adds no new warnings.
- Each task ends with `cargo test -p models -p python-extractor -p java-extractor -p go-extractor -p extractor-runtime -p synthesizer -p cli` green.
- Commit per task, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. `openspec/` stays uncommitted until the user decides.
- Scoring: Zenodo v2 `~/general-notes/research/voyantclair/paper/zenodo-artifact-v2/scoring/score.py` only (empaia oracle `scoring/empaia-ground-truth/sdg-edges.json`, 17 edges).

## 1. Baseline

- [x] 1.1 Build HEAD (`cargo build -p cli --release`) and run static (no `--llm`, no `--typesafe`) on train-ticket, empaia (`-f ./config/constants/empaia-constants.json`) and train-ticket-polyglot (`config/configurations/local-train-ticket-polyglot-config.json`) into `jev_triage_runs/prefix-v1/base-<corpus>/`. Verify: empaia and train-ticket reproduce the `static+fuzzy` row of `jev_triage_runs/matcher-v1/results.md` (TT service 1.000/0.967/0.983, empaia 0.941/0.941/0.941).
- [x] 1.2 Score service level for all three (polyglot oracle `systems/train-ticket-polyglot/benchmark-ground-truth`), endpoint level (`jev_triage_runs/score_endpoints.py`) for train-ticket and polyglot. Record per corpus: endpoint URI set size, empaia endpoints of mounted services whose URI does not start with a mount prefix, tag counts, calls with >1 candidate, empaia v1/v3 check ("correct dropped" / "correct + extra", script in matcher-v1 Task 8). Write `jev_triage_runs/prefix-v1/results.md` row `base`. Verify: row present.

## 2. Neutral model (D1, D2)

- [x] 2.1 Add `RouteFact` (D1) to `models` (new `models/src/route_facts.rs`, re-exported like the other model types) and `Endpoint.enclosing_function` (D2). Set `enclosing_function: None` in every Java, Go, Django constructor and every literal `Endpoint { .. }` in tests. Verify: full test list green; `cargo build -p synthesizer` (bolt mapping) compiles.
- [x] 2.2 Failing test `decorator_endpoint_records_enclosing_function` in `python-extractor/tests/python/endpoints.rs`: `def add_routes_clinical(app):\n    @app.get("/cases/{case_id}")\n    async def _(case_id: str): ...` -> `router_variable == Some("app")`, `enclosing_function == Some("add_routes_clinical")`; module-level `@app.get("/alive")` -> `None`. Then fill it in the Python decorator / method-decorator / flask strategies via `find_enclosing_information` (make it shared, e.g. `pub(crate)` in `common.rs`). Verify: test fails first, then passes.
- [x] 2.3 Commit `feat(models): neutral route facts and endpoint enclosing function`.

## 3. Shared resolver (D3-D6)

- [x] 3.1 Failing tests in new `extractor-runtime/src/pipeline/pass2/route_prefixes.rs` (`#[cfg(test)] mod tests`). Inputs are hand-built `TypedFileRecord`s (only `file_path`, `endpoints`, `callables`, `call_statements`, `assignments` filled) plus `Vec<RouteFact>` per file and an `ImportGraph`; no parser. Expose `fn resolve_with_facts(files: &mut [TypedFileRecord], facts: &[(usize /*file idx*/, Vec<RouteFact>)], graph: &ImportGraph)` for this; `resolve_route_prefixes` = adapters + `resolve_with_facts`. Assert sorted `(file, uri)`:
  - `attach_prefix`: Attach{app <- sub, "\"/v3\""}, endpoint `/x` on `sub` -> `/v3/x`.
  - `own_then_outer_prefix`: Own{r, "\"/items\""} + Attach{app <- r, "\"/api\""} -> `/api/items/{id}`.
  - `prefix_from_global_constant`: Own{router, "BASE_PATH"} in `Function(create_router)`, Global assignment `BASE_PATH = "/api/v1/consignservice"` -> `/api/v1/consignservice/consigns`.
  - `prefix_from_imported_constant`: `BASE_PATH` defined in another file, resolved through `ImportGraph`.
  - `param_followed_across_two_calls`: three files `svc/app.py` (Attach{app <- app_v1, "/v1"}, call `add_routes_v1(app_v1)`), `svc/api/v1/__init__.py` (callable `add_routes_v1(app)`, call `add_routes_clinical(app)`), `svc/api/v1/clinical.py` (callable `add_routes_clinical(app)`, endpoint on `app`, enclosing `add_routes_clinical`) -> `/v1/cases/{case_id}`.
  - `callee_via_import_graph_beats_name_match`: two callables named `add_routes_v1` in one service; import graph points to one -> only its prefix.
  - `same_helper_name_other_service_not_mixed`: shape above under `a/` (`/v1`) and `b/` (`/v9`), empty import graph -> no mixing.
  - `helper_reused_by_two_mounts_yields_two_endpoints` -> `/v1/alive`, `/v3/alive`.
  - `unresolved_prefix_keeps_uri`: Own{r, "settings.base"} -> `/x`.
  - `recursive_helper_terminates` -> finishes, `/v1/x`.
  - `no_facts_no_change`: endpoint without `router_variable` (Java-like) untouched.
  - `output_independent_of_file_order`.
  Verify: `cargo test -p extractor-runtime route_prefixes` fails.
- [x] 3.2 Implement D3-D6; wire `resolve_route_prefixes(&mut files, &import_graph)` into `build_project_ir` after `resolve_call_argument_types`. Adapter dispatch returns empty for every language in this task. Verify: 3.1 tests pass, full test list green, train-ticket static run endpoint URI set equals base.
- [x] 3.3 Commit `feat(extractor-runtime): language-agnostic route prefix resolver`.

## 4. Python FastAPI adapter

- [x] 4.1 Failing tests in new `python-extractor/src/extraction/route_facts.rs`, parsing real Python through the same per-file extraction pass 1 uses:
  - `mount_positional_and_keyword`: `app.mount("/v1", app_v1)`, `app.mount(path="/v3/fhir", app=app_v3)` -> two `Attach`.
  - `include_router_with_and_without_prefix`: `app.include_router(r, prefix="/api")` -> Attach prefix `"/api"`; `app.include_router(r)` -> Attach prefix `""`.
  - `api_router_own_prefix`: `router = APIRouter(prefix=BASE_PATH)` inside `def create_router(s)` -> Own{scope Function(create_router), router, "BASE_PATH"}.
  - `unrelated_calls_ignored`: `client.mount_volume(x)`, `APIRouter()` without prefix -> no facts.
  Verify: tests fail first.
- [x] 4.2 Implement `pub fn route_facts(file: &TypedFileRecord) -> Vec<RouteFact>` from `call_statements` (`*.mount`, `*.include_router`) and `assignments` (`APIRouter(` with `prefix=`); hook it into the pass 2 adapter dispatch for `Language::Python`. Add one end-to-end test in `extractor-runtime/tests/pipeline/` running pass 1 + 2 on the empaia three-file shape and the polyglot `create_router` shape. Verify: tests pass, full list green.
- [x] 4.3 Commit `feat(python-extractor): FastAPI route fact adapter`.

## 5. Docs

- [x] 5.1 Add "Route prefixes" to `docs/endpoint_restcall_matching.md`: neutral facts, resolver rules, how to add an adapter (one function + dispatch line), current adapters, non-goals. Verify: section present, no Unicode arrows. Commit `docs: route prefix resolution`.

## 6. Measure

- [x] 6.1 Rerun 1.1 into `jev_triage_runs/prefix-v1/prefix-<corpus>/`, score as 1.2, add row `prefix`. Verify:
  - train-ticket Java endpoint URI set and scores identical to `base`;
  - empaia: mounted-service endpoints carry their mount prefix (list any that do not, with cause); v1/v3 "correct dropped" <= base; `suffix` count and calls with >1 candidate drop;
  - polyglot consign / travel-plan endpoints start with `/api/v1/`;
  - service-level recall >= base on all three.
  Stop and report to the user if a check fails.
- [x] 6.2 Jev check (costs credits, ask the user first): empaia `--typesafe --jev-match` into `prefix-jev-empaia/`; record Jev calls, input tokens, v1/v3 "correct dropped". Expect far below 296 calls / 161k tokens / 34 dropped.
- [x] 6.3 Report to user: table base vs prefix (and prefix+jev), remaining ambiguous calls with causes, next adapter candidates, recommendation on the Jev option-text follow-up.
