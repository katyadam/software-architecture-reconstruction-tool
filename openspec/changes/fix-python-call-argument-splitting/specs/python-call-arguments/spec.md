# Spec Delta

## Purpose

Defines how the Python extractor splits a call's argument list into positional and keyword arguments, and which argument is used as an outbound REST call's target URL.

## ADDED Requirements

### Requirement: Arguments follow Python syntax
The extractor SHALL produce exactly one argument per top-level argument of a call. A keyword argument SHALL be split into name and value only at its own `=`. Commas and `=` inside strings, brackets or nested calls SHALL NOT split an argument. Comments SHALL NOT produce arguments.

#### Scenario: Query string in an inline URL
- **WHEN** the code contains `requests.get(BASE + "/x?id=1")`
- **THEN** the call has one positional argument with value `BASE + "/x?id=1"`

#### Scenario: Comma in a string literal
- **WHEN** the code contains `requests.get(BASE + "/a,b")`
- **THEN** the call has one positional argument with value `BASE + "/a,b"`

#### Scenario: Keyword arguments
- **WHEN** the code contains `requests.get(timeout=5, url=BASE + "/x")`
- **THEN** the call has the keyword arguments `timeout` = `5` and `url` = `BASE + "/x"`

### Requirement: REST call target argument
The target of an outbound REST call SHALL be the value of the `url` keyword argument when one is given, otherwise the first argument.

#### Scenario: URL passed as keyword after another keyword
- **WHEN** the code contains `requests.get(timeout=5, url=BASE + "/x")` inside a function
- **THEN** the REST call target is built from `BASE + "/x"`, not `5`

#### Scenario: Inline URL with a query string or comma is not cut
- **WHEN** a function calls `requests.get(BASE + "/x?id=1")` or `requests.get(BASE + "/a,b")`
- **THEN** a REST call is identified and its target ends with `/x?id=1` or `/a,b` respectively
