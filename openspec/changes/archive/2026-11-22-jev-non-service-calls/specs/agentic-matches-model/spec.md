## MODIFIED Requirements

### Requirement: Excluded points are kept for audit
Points deliberately left out of the SDG SHALL be listed as excluded, with a reason, note and evidence. A point talking to a system outside the analysed project SHALL be excluded with reason `External`. A point that makes no network call at all, such as a database layer, cache or SDK call mistaken for a REST call, SHALL be excluded with reason `InProcess`.

#### Scenario: Third-party API
- **WHEN** an initiator calls a third-party API outside the project
- **THEN** it appears in the excluded list with reason `External`, not in the unmatched list or the SDG

#### Scenario: Cache call mistaken for REST
- **WHEN** an initiator is a cache `.get(key)` call
- **THEN** it appears in the excluded list with reason `InProcess`, not in the unmatched list or the SDG
