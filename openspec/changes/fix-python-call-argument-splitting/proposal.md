# Proposal

## Why

The Python extractor builds a call's arguments by splitting the argument text on `,` and `=`. A URL that contains a query string or a comma, or a call that passes the URL as a keyword after another keyword, is cut apart, so REST calls are lost or get a wrong target (issue #59):

- `requests.get(BASE + "/x?id=1")` -> no REST call
- `requests.get(BASE + "/a,b")` -> target `"/a`
- `requests.get(timeout=5, url=BASE + "/x")` -> target `5`

The same URLs resolve correctly when first assigned to a variable, so inline and variable forms disagree.

## What Changes

- Call arguments are taken from the parsed argument list: one argument per top-level argument, keyword arguments split only at their own `=`, comments ignored. Commas and `=` inside strings, nested calls or brackets no longer split an argument.
- A REST call's target is the `url` keyword argument when present, otherwise the first argument (as before).

## Capabilities

### New Capabilities
- `python-call-arguments`: how the Python extractor turns a call's argument list into arguments, and which argument is a REST call's target.

### Modified Capabilities

## Impact

- `python-extractor/src/extraction/calls/extractor.rs` (argument building)
- `python-extractor/src/extraction/restcalls/identification/method_call.rs` (target selection)
- Kafka/RabbitMQ edge extraction reads the same arguments by name and position; their behaviour is unchanged for well-formed calls.
