## Why

A Python REST call written inline as `requests.get(BASE + "/x")` loses `BASE`: extraction records the target as `/x`, so pass 3 has nothing to evaluate. The same call written `url = BASE + "/x"; requests.get(url)` works. Found during `route-prefix-eval-pass3`, whose spec scenarios had to use the variable form.

## What Changes

- REST call target identification in the Python extractor keeps every operand of a `+` concatenation of string literals and dotted names whose first operand is not a string literal.
- String literal operands are normalized to the form pass 3 evaluates. Dotted names become `{name}`, which resolves to a known value or stays a hole for matching.
- Pass 3 resolves a whole-part `{name}` like a bare name, and no longer strips a leading `f` from plain values.
- `statix::strings::clean_python_string` does not change. Its other callers (decorators, Flask, Django, Kafka, RabbitMQ, the Python parser) are unaffected.

## Capabilities

### New Capabilities
- `python-restcall-targets`: which operands of an inline Python REST call target are kept for evaluation.

### Modified Capabilities
(none)

## Impact

- `python-extractor/src/extraction/restcalls/identification/method_call.rs`: `identify_target_uri`.
- `python-extractor/src/extraction/restcalls/evaluation/uri_generator.rs`: `get_resolved_parts`.
- Tests in `python-extractor/tests/python/restcalls.rs` and `extractor-runtime/tests/pipeline/framework_endpoints.rs`.
- Corpora: 3 polyglot sites move from `exact`/`template` to `suffix`, same edges. Empaia's 1 site (`service['url'] + '/alive'`) keeps today's path.
- `route-prefix-eval-pass3` must be applied first: its env precedence is what these tests rely on for same-named constants.
