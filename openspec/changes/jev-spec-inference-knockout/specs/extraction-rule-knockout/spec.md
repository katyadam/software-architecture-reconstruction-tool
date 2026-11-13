## Purpose

Lets an experiment switch off named static extraction rules, so the recall a rule provides can be measured and other mechanisms can be tested on what it no longer finds.

## ADDED Requirements

### Requirement: Named rules can be disabled
The system SHALL accept a set of rule names to disable for one run, and SHALL then not report any communication element that only the disabled rules would have identified.

#### Scenario: RestTemplate knockout
- **WHEN** the CLI runs on train-ticket with `--knockout java-resttemplate`
- **THEN** no REST call identified through Spring `RestTemplate` appears in the output

#### Scenario: Python method-name knockout
- **WHEN** the CLI runs with `--knockout python-http-method-names`
- **THEN** no Python REST call recognized only by its method name (`get`, `post`, ...) appears in the output

### Requirement: Unknown rule names are rejected
The system SHALL reject a run that names a rule it does not know, listing the known rule names.

#### Scenario: Typo in rule name
- **WHEN** the CLI runs with `--knockout java-rest-template`
- **THEN** the run stops before extraction with an error that lists `java-resttemplate` and `python-http-method-names`

### Requirement: No knockout means unchanged output
The system SHALL produce the same output as before this change when no rule is disabled.

#### Scenario: Default run
- **WHEN** the CLI runs on train-ticket without `--knockout`
- **THEN** `sdg.json` contains the same set of requests as before this change
