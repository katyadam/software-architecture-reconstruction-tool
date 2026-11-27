# Spec Delta

## Purpose

Defines how symbolic evaluation chooses the callable a call refers to, so results are reproducible across runs.

## ADDED Requirements

### Requirement: Deterministic callee choice
When more than one callable matches a call equally well, symbolic evaluation SHALL choose the same callable on every run with the same input, independent of hash map iteration order.

#### Scenario: Two callables match a call exactly
- **WHEN** a Python file defines `base() -> str` returning `"http://a"` and `base()` returning `"http://b"`, and another function returns `base() + "/x"`
- **THEN** evaluating that function gives the same value on every run

#### Scenario: Single best match
- **WHEN** only one callable matches a call best
- **THEN** that callable is chosen, as before

### Requirement: Callee arity matches the call
Symbolic evaluation SHALL only resolve a call to a callable with the same number of parameters as the call has arguments.

#### Scenario: Argument of unknown type
- **WHEN** a Python file defines `f()` returning `"http://a"` and `f(x)` returning `"http://b"`, and another function returns `f(unknown)`
- **THEN** evaluating that function gives `"http://b"`
