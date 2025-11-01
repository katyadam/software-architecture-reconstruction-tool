pub const CREATE_IMCG: &str = r#"
UNWIND $callables AS callable
MERGE (c:Callable {id: callable.signature, codebase_uuid: $codebase_uuid})
SET c.codebase_uuid = $codebase_uuid
SET c.namespace = callable.namespace
SET c.parameters = callable.parameters
SET c.return_type = callable.return_type
SET c.is_async = callable.is_async
SET c.is_constructor = callable.is_constructor
SET c.hash = callable.hash

WITH $calls AS calls
UNWIND calls AS call
MATCH (src:Callable {id: call.source_id, codebase_uuid: $codebase_uuid})
MATCH (tgt:Callable {id: call.target_id, codebase_uuid: $codebase_uuid})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid}]->(tgt)
SET r.request = call.request
"#;

pub const GET_IMCG: &str = r#"
MATCH (c:Callable {codebase_uuid: $codebase_uuid})
OPTIONAL MATCH (c)-[r:DEPENDS_ON]->(target:Callable)
WHERE target.codebase_uuid = $codebase_uuid
WITH collect(DISTINCT c) AS all_callables, collect(DISTINCT {source: c.id, target: target.id}) AS all_calls
RETURN all_callables, all_calls
"#;

pub const DELETE_IMCG: &str = r#"
MATCH (c:Callable {codebase_uuid: $codebase_uuid})
DETACH DELETE c
"#;
