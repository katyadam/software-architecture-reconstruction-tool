## MODIFIED Requirements

### Requirement: Output files
The run SHALL write, under the output directory:
- `match/sdg.json`: the SDG of `Matches`, all protocols.
- `match/unmatched.json`: the unmatched points, all protocols.
- `match/excluded.json`: the excluded points, all protocols, sorted by point id. Without `--jev`, it holds an empty list.
- `match/sdg.legacy.json`: the HTTP dependencies of the SDG in the synthesizer's JSON shape (`services`, `connections` with `source_id`, `target_id`, `kind`, `requests` of `endpoint` and `restcall`, and an empty `message_connections`). The `restcall` SHALL be the original element.
- `match/metrics.json`: the counts below.
- `final/sdg.json` and `final/sdg.legacy.json`: copies of the match files, since no later stage ran.
Services and dependencies SHALL be sorted by name and protocol, so two runs on the same input give identical files.

#### Scenario: Scoreable output
- **WHEN** score.py reads `final/sdg.legacy.json`
- **THEN** it parses without error

#### Scenario: Deterministic output
- **WHEN** the run is repeated on the same input, in any checkout
- **THEN** every output file is byte-identical

#### Scenario: Static run has no excluded points
- **WHEN** the run is given no `--jev`
- **THEN** `match/excluded.json` holds an empty list
