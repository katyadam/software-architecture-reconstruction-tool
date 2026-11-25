# Spec Delta

## Purpose

Defines how the Python extractor turns a REST call target template into concrete URIs, including robustness against degenerate template parts.

## ADDED Requirements

### Requirement: Degenerate quote parts do not abort URI resolution
URI resolution SHALL NOT panic or abort extraction when a part of a target template consists of a single `"` character. Such a part SHALL be kept verbatim instead of being treated as a string literal.

#### Scenario: Lone quote part from a literal containing a plus sign
- **WHEN** the target template is `"a+" + x`
- **THEN** URI resolution completes and returns exactly one URI

### Requirement: Empty string literals resolve to an empty string
A template part that is an empty quoted string literal (`""`) SHALL contribute an empty string to the resolved URI.

#### Scenario: Empty literal concatenated with a path
- **WHEN** the target template is `"" + "/api"`
- **THEN** URI resolution returns exactly `/api`
