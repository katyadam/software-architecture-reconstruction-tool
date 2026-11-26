# Proposal

## Why

Repeated runs on the same input can resolve the same Python REST call to different URLs (issue #62). When symbolic evaluation meets a call such as `base()`, it picks the callee by scanning a hash map and taking the first good match. Several callables can match equally well, and hash map order changes from one run to the next. So the chosen callee, and the URL built from its return value, change too.

## What Changes

- When several callables match a call equally well, the matcher always picks the same one: the candidate with the smallest key.
- Calls with a single best match resolve as before.
- A call never resolves to a callable with a different number of parameters (before, `f(<unknown>)` could resolve to `f()`).

## Capabilities

### New Capabilities
- `callee-resolution`: How symbolic evaluation picks the callable that a call refers to.

### Modified Capabilities

## Impact

- Code: `statix/src/matcher.rs` (`find_closest_callable_impl`), shared by the Python and Java matchers.
- Output: REST call URLs and other symbolically evaluated values no longer change between runs on the same input. Where a run used to pick another candidate, the result may differ from that run.
- Not covered: other first-wins choices over hash maps (project constant collection, external constant matching). They are listed as remaining suspects for #62.
