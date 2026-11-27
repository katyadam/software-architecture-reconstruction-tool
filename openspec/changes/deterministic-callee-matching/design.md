# Design

## Context

`find_closest_callable_impl` loops over `callables.keys()` of a `HashMap`. It returns the first key whose mangled form (return type stripped) equals the call, or else keeps the first key with the highest count of matching parameter types. Python keys include the return type (`str base()`, `Any base()`), so one call can match several keys exactly. In the fallback, ties are common because Python argument types are often unknown. `HashMap` uses a random seed per process, so the first match differs between runs.

## Goals / Non-Goals

**Goals:**
- The same input always picks the same callee.

**Non-Goals:**
- Picking a "better" callee (for example, preferring the caller's own file). This needs more context than the matcher has.
- Other first-wins choices over hash maps in `extractor-runtime` (constants). Tracked under #62.

## Decisions

- Tie-break by the smallest key, both for exact matches and for equal fallback scores. This is cheap and needs no new data. Alternatives: sort all keys on each call (costs O(n log n) per call on a project-wide map), or switch the callables map to `BTreeMap` (touches many signatures across crates). Both are bigger than needed.
- Rank candidates in one pass: same name and arity, then most matching parameter types, then smallest key. An exact match is the case where every type matches, so it still wins, and each header is parsed once instead of twice. Review follow-up: the old exact check compared joined type strings, so `f(<unknown>)` matched `f()`. Filtering by arity first removes that wrong match too.

## Risks / Trade-offs

- [The smallest key may not be the callee the code really calls] → Accepted. Before, the pick was random. Now it is stable, which makes scores repeatable.
- [Full scan on calls with an exact match, which used to stop early] → Linear over the map, the same as the fallback path. Each header is now parsed once.
