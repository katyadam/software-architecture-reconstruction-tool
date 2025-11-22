pub const CREATE_CONTEXT_MAP: &str = r#"
CALL {
    MATCH (e:Entity {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
    DETACH DELETE e
}

UNWIND $entities AS entity
MERGE (e:Entity {id: entity.signature, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
SET e.name = entity.name
SET e.superclasses = entity.superclasses
SET e.fields = entity.fields
SET e.service_name = entity.service_name

WITH $dependencies AS dependencies
UNWIND dependencies AS dep
MATCH (src:Entity {id: dep.source_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MATCH (tgt:Entity {id: dep.target_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash}]->(tgt)
"#;

pub const GET_CONTEXT_MAP: &str = r#"
MATCH (e:Entity {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
OPTIONAL MATCH (e)-[r:DEPENDS_ON]->(target:Entity)
WHERE target.codebase_uuid = $codebase_uuid AND target.commit_hash = $commit_hash
WITH collect(DISTINCT e) AS all_entities, collect(DISTINCT {source: e.id, target: target.id}) AS all_dependencies
RETURN all_entities, all_dependencies
"#;

pub const DELETE_CONTEXT_MAP: &str = r#"
MATCH (e:Entity {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
DETACH DELETE e
"#;
