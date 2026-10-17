# Endpoint-RestCall matching

## Problem

- How to solve a problem finding a REST call correct target endpoint?
- You can match directly (endpointUrl == restCallTargetUrl).
- This is not feasible, often endpoint doesn't contain full url specification.
- Also service can be deployed in a way that it is accessible by multiple URLs - docker network, localhost, etc...

## Solution

- Matching unifies path segments (`synthesizer/src/sdg/matching.rs`); Levenshtein only as a last-resort fallback.
- Each service in configuration can contain multiple urls that it is accesible from.

### Algorithm

1. Method filter: only endpoints with the call's HTTP method.
2. Host scope: reflexive call (localhost / own url) -> own service only; host owned by a configured service -> that service; unknown host -> every other service.
3. Segment unification: same length, each segment pair equal or one side a variable (`{x}`, `:x`, `*`) -> `exact` (all literal) or `template`.
4. Suffix: call starting with a hole (`{HOST}/a/b`) or a known host with extra leading segments (lost router prefix, e.g. `/v3`) matches the endpoint on a trailing run -> `suffix`.
5. Evidence: drop a candidate with no aligned position literal on both sides (`{requestUrl}`, `{a}/{b}` unify with anything).
6. Suffix candidates dropped when any `exact` / `template` candidate exists.
7. Specificity pruning: drop a candidate whose literal-matched positions are a strict subset of another's.
8. Fuzzy fallback: nothing left -> Levenshtein argmin over the scoped endpoints (uri prefixed with each service url, or bare); kept if distance < 0.3 x the longer of endpoint uri and call uri; ties -> smallest sort key; at most one candidate, tag `fuzzy` (catches glued hosts like `http://svc:5000url`).
9. Deterministic sort by service name, uri, function name, file path.
10. One `Request` per surviving candidate, tagged in field `match` (`exact`, `template`, `suffix`, `fuzzy`).
