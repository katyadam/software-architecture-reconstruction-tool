# Spec Delta

## ADDED Requirements

### Requirement: Only a real f-string prefix is stripped
URI resolution SHALL remove a leading `f` from a target part only when a quote (`"` or `'`) follows it directly, which marks a Python f-string literal. A resolved value that starts with `f` but is not f-string source SHALL keep its leading `f`.

#### Scenario: Variable value starting with f keeps its first letter
- **WHEN** a variable `HOST` resolves to `frontend/{x}` and the target template is `HOST + "/y"`
- **THEN** URI resolution returns `frontend/{x}/y`

#### Scenario: f-string target loses only its prefix
- **WHEN** the target template is the f-string `f"frontend/{id}"` and `id` is unknown
- **THEN** URI resolution returns `frontend/{id}`
