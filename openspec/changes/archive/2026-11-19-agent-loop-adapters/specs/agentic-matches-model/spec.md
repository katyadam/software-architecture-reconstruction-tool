## REMOVED Requirements

### Requirement: Evidence cites recorded facts
**Reason**: Answers no longer cite facts, so nothing pairs a claim with one fact. The `Address` / `TargetService` / `Receiver` pairing was never backed by the model's output.
**Migration**: Evidence is now the list of facts a session recorded. See "Evidence is the session's facts".

## ADDED Requirements

### Requirement: Evidence is the session's facts
The evidence of a link, an unmatched point, an excluded point or a verdict SHALL be the list of facts recorded by the session that produced it, in fetch order. A fact SHALL store its id, tool name, arguments and tool output. Fact text SHALL be written by the runtime, never by the model.

#### Scenario: Saved fact outlives the session
- **WHEN** a later stage reads a link's evidence
- **THEN** it sees the tool output itself, without access to the earlier model session

#### Scenario: Facts in fetch order
- **WHEN** the matcher session for a link recorded `f1` then `f2`
- **THEN** the link's evidence is `f1` then `f2`

#### Scenario: Answer without facts
- **WHEN** a session answers on its first call
- **THEN** the result's evidence is empty
