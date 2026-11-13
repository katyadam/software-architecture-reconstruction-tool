## Why

Static extraction is exact for frameworks we hand-wrote rules for (train-ticket endpoint precision 1.000) and blind to the rest: voucher -> order is missed because `urllib.request` is not a known client (spec #15); a new corpus with an unknown HTTP library loses every edge through it. Asking Jev at every call site would fix that but costs tokens per call site (~540 tokens per question measured in matcher-v1 Task 8) and breaks run-to-run determinism.

Hypothesis: Jev can learn **per library symbol** what static rules encode by hand ("`RestTemplate.exchange` is an HTTP client, URL = argument 0, method = argument 1"), once per project, cached, then applied statically. Cost then scales with distinct library symbols, not call sites, and cached runs are deterministic.

This change is the experiment that tests the hypothesis before building more: knock out a static rule we already have, and measure how much of the lost recall Jev spec inference recovers, at what cost and precision.

## What Changes

- **Knockout switch**: `--knockout <rule>` (repeatable) disables a named static identification rule. Initial rules: `java-resttemplate` (Spring `RestTemplate` client calls), `python-http-method-names` (Python client calls recognized by method name `get` / `post` / ...). Without the flag, output is unchanged.
- **Client spec inference** behind `--jev-specs` (requires `--typesafe`):
  - static selects the residue: call statements no static rule claimed whose callee is a library symbol and whose arguments look like a URL;
  - groups them by callee symbol and asks Jev once per symbol, with a DDFA-style fact loop (sample call sites, import, receiver type, signature);
  - Jev answers: is it an outbound HTTP call, which argument is the URL, where the HTTP method comes from;
  - accepted answers become `ClientSpec`s, cached in a JSON file, and are applied statically to every call site of that symbol;
  - inferred REST calls carry provenance `spec` so scoring can separate them.
- **Experiment runs and report** (no product decision yet): knockout vs knockout + spec inference vs static, on train-ticket, train-ticket-polyglot and empaia; plus a natural run (no knockout) to see whether `urllib` is recovered.
- Endpoint / route spec inference is a follow-up arm, decided after this one, and builds on `endpoint-route-prefixes` (a learned spec = a route-fact adapter written as data).

## Capabilities

### New Capabilities
- `communication-spec-inference`: learn per-library communication specs with Jev for call sites static rules do not recognize, cache them, apply them deterministically, and expose provenance and cost.
- `extraction-rule-knockout`: disable named static extraction rules for ablation experiments.

### Modified Capabilities
<!-- none: no existing specs in openspec/specs/ -->

## Impact

- `models`: `ClientSpec`; `RestCall.origin` (`static` default / `spec`), serde default.
- `java-extractor`, `python-extractor`: identification rules gated by a knockout set.
- `extractor-runtime`: new async stage between pass 2 and pass 3 (`spec_inference`), using the existing TypeSafe client; spec cache read/write.
- `clients`: one Jev question set for client specs (in `clients/src/typesafe/question/`).
- `cli`: `--knockout`, `--jev-specs`, `--spec-cache <file>`.
- Costs Jev credits only with `--jev-specs`. Runs under `jev_triage_runs/spec-knockout-v1/`.
