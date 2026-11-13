## Purpose

Python REST call targets written as an inline string concatenation keep all their operands, so they resolve the same way as the equivalent assignment to a variable.

## ADDED Requirements

### Requirement: Inline concatenation keeps its operands
The system SHALL resolve a Python REST call target written as a top-level `+` concatenation with a non-literal first operand, whose non-literal operands are all dotted names, to the same URL as the equivalent `url = <expression>; client.method(url)` when every operand has a known value. An operand without a known value SHALL stay in the target as a `{name}` hole.

#### Scenario: Constant plus literal
- **WHEN** a file declares `BASE = "http://svc:8000"` and calls `requests.get(BASE + "/x")`
- **THEN** the REST call target is `http://svc:8000/x`

#### Scenario: Single-quoted literal
- **WHEN** a file declares `BASE = "http://svc:8000"` and calls `requests.get(BASE + '/x')`
- **THEN** the REST call target is `http://svc:8000/x`

#### Scenario: Literal between two names
- **WHEN** a file declares `BASE = "http://svc:8000"` and `TAIL = "y"` and calls `requests.get(BASE + "/x/" + TAIL)`
- **THEN** the REST call target is `http://svc:8000/x/y`

#### Scenario: f-string operand
- **WHEN** a file declares `BASE = "http://svc:8000"` and a function with parameter `item` calls `requests.get(BASE + f"/x/{item}")`
- **THEN** the REST call target is `http://svc:8000/x/{item}`

#### Scenario: Unresolved operand
- **WHEN** a function calls `requests.get(self.base + "/x")` and `self.base` has no known value
- **THEN** the REST call target is `{self.base}/x`, so `self.base` stays a hole that matching can align by suffix

#### Scenario: Same constant name in two services
- **WHEN** two services each declare `BASE` with different values and a file in the second service calls `requests.get(BASE + "/x")`
- **THEN** the REST call target uses the second service's value

### Requirement: Non-URL lookups stay excluded
The system SHALL NOT report a REST call when no string literal in the target contains `/` and the target contains a string literal.

#### Scenario: Dictionary lookup with a concatenated key
- **WHEN** a function calls `settings.get(PREFIX + "_key")`
- **THEN** no REST call is reported

### Requirement: Other target forms are unchanged
The system SHALL extract targets unchanged when the first operand is a string literal, when there is no top-level `+`, when a non-literal operand is not a dotted name, when a literal operand is followed by more text, and for all non-REST uses of string cleaning.

#### Scenario: Literal first
- **WHEN** a function calls `requests.get("http://svc:8000/" + path)`
- **THEN** the raw target is the same as before this change

#### Scenario: Bare variable
- **WHEN** a function calls `requests.get(url)`
- **THEN** the raw target is `url`

#### Scenario: Operand that is not a dotted name
- **WHEN** a function calls `requests.get(lookup(x) + "/y")`
- **THEN** the raw target is the same as before this change

#### Scenario: Plus inside a nested call
- **WHEN** a function calls `requests.get(build("a" + b))`
- **THEN** the raw target is the same as before this change
