pub const CREATE_CONTEXT_MAP: &str = r#"
UNWIND $entities AS entity
MERGE (e:Entity {id: entity.signature, codebase_uuid: $codebase_uuid})
SET e.id = entity.signature
SET e.name = entity.name
SET e.superclasses = entity.superclasses
SET e.fields = entity.fields
SET e.service_name = entity.service_name
SET e.codebase_uuid = $codebase_uuid

WITH $dependencies AS dependencies
UNWIND dependencies AS dep
MATCH (src:Entity {id: dep.source_id, codebase_uuid: $codebase_uuid})
MATCH (tgt:Entity {id: dep.target_id, codebase_uuid: $codebase_uuid})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid}]->(tgt)
"#;

pub const GET_CONTEXT_MAP: &str = r#"
MATCH (e:Entity {codebase_uuid: $codebase_uuid})
OPTIONAL MATCH (e)-[r:DEPENDS_ON]->(target:Entity)
WHERE target.codebase_uuid = $codebase_uuid
WITH collect(DISTINCT e) AS all_entities, collect(DISTINCT {source: e.id, target: target.id}) AS all_dependencies
RETURN all_entities, all_dependencies
"#;

pub const DELETE_CONTEXT_MAP: &str = r#"
MATCH (e:Entity {codebase_uuid: $codebase_uuid})
DETACH DELETE e
"#;
