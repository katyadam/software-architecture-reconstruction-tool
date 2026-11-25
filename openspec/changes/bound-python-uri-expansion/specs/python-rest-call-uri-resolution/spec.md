# Spec Delta

## ADDED Requirements

### Requirement: URI expansion per call target is bounded
URI resolution SHALL return at most 256 URIs for a single REST call target, however many combinations its parts allow. When the combinations exceed the limit, the URIs kept SHALL be the first ones in combination order (parts left to right, each part's values in their resolved order), so the same input always yields the same output.

#### Scenario: Exponential combination count is capped
- **WHEN** the target template concatenates 20 variables that each resolve to 2 values (2^20 combinations)
- **THEN** URI resolution completes and returns exactly 256 URIs
- **AND** the first URI combines the first value of every part

#### Scenario: Small expansions are unchanged
- **WHEN** the target template concatenates two variables that resolve to 2 and 3 values
- **THEN** URI resolution returns all 6 combinations
