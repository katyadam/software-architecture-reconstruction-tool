## Purpose

Recovers service communication through libraries that have no hand-written extraction rule, by having Jev learn one reusable specification per library symbol and applying it to every call site, at a cost that grows with the number of distinct library symbols rather than with call sites.

## ADDED Requirements

### Requirement: Only unrecognized URL-carrying library calls are candidates
The system SHALL consider for inference only call statements that no active static rule identified, whose callee resolves to a library symbol, and that pass a URL-like argument (a string containing `/` or `://`, or an identifier whose name or resolved value looks like a URL). Calls to functions defined in the analyzed project SHALL NOT be candidates.

#### Scenario: Unrecognized client call is a candidate
- **WHEN** a Python file calls `urllib.request.urlopen(order_url)` and `order_url` resolves to `http://ts-order-service:12031/api/v1/...`
- **THEN** `urllib.request.urlopen` is a candidate symbol

#### Scenario: Recognized call is not re-asked
- **WHEN** a call is already identified as a REST call by an active static rule
- **THEN** its callee is not sent to Jev

### Requirement: One question per library symbol
The system SHALL ask Jev about each candidate symbol at most once per run, with facts gathered from a bounded sample of its call sites, regardless of how many call sites use it.

#### Scenario: Symbol used at many call sites
- **WHEN** `RestTemplate.exchange` appears at 120 call sites
- **THEN** Jev is asked about `RestTemplate.exchange` once

### Requirement: Accepted specs are applied to every call site
The system SHALL turn an answer that the symbol is an outbound HTTP call, with an identified URL argument and method source, into a client spec, and SHALL apply that spec statically to every call site of the symbol, producing REST calls that then go through the normal URI evaluation and matching.

#### Scenario: Spec recovers knocked-out calls
- **WHEN** `java-resttemplate` is knocked out and Jev answers that `RestTemplate.exchange` is an HTTP client with URL in argument 0 and method in argument 1
- **THEN** every `exchange` call site becomes a REST call with the URI from argument 0 and the method from argument 1

#### Scenario: Rejected symbol produces nothing
- **WHEN** Jev answers that a candidate symbol is not an outbound HTTP call, or its confidence is below the acceptance threshold
- **THEN** no REST call is produced from that symbol

### Requirement: Specs are cached and reused
The system SHALL store accepted and rejected answers in a spec cache file keyed by symbol, and SHALL use the cache instead of asking Jev when an entry exists, so a second run with the same cache asks nothing and produces identical output.

#### Scenario: Second run with cache
- **WHEN** a run with `--jev-specs --spec-cache specs.json` is repeated with the same cache file
- **THEN** no Jev call is made and `sdg.json` contains the same set of requests as the first run

### Requirement: Provenance and cost are visible
The system SHALL mark REST calls produced from inferred specs with provenance `spec`, and SHALL report the number of Jev calls, input tokens, candidate symbols, and accepted specs per run.

#### Scenario: Provenance in output
- **WHEN** a REST call is produced from an inferred spec
- **THEN** its serialized form carries `"origin": "spec"`, and statically identified calls carry `"origin": "static"` or no origin field

### Requirement: Inference is opt-in
The system SHALL NOT call Jev for spec inference unless explicitly enabled, and enabling it SHALL require TypeSafe credentials.

#### Scenario: Static run
- **WHEN** the CLI runs without `--jev-specs`
- **THEN** no Jev call is made and the output is unchanged by this capability
