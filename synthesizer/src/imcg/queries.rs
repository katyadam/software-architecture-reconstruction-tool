pub const CREATE_IMCG: &str = r#"
CALL {
    MATCH (c:Callable {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
    DETACH DELETE c
}

UNWIND $callables AS callable
MERGE (c:Callable {id: callable.signature, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
SET c.name = callable.name
SET c.namespace = callable.namespace
SET c.parameters = callable.parameters
SET c.return_type = callable.return_type
SET c.is_async = callable.is_async
SET c.is_constructor = callable.is_constructor
SET c.hash = callable.hash
SET c.service_name = callable.service_name

WITH $calls AS calls
UNWIND calls AS call
MATCH (src:Callable {id: call.source_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MATCH (tgt:Callable {id: call.target_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash}]->(tgt)
SET r.request = call.request
SET r.message_request = call.message_request
"#;

pub const GET_IMCG: &str = r#"
MATCH (c:Callable {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
OPTIONAL MATCH (c)-[r:DEPENDS_ON]->(target:Callable)
WHERE target.codebase_uuid = $codebase_uuid AND target.commit_hash = $commit_hash
WITH 
    collect(DISTINCT c) AS all_callables,
    collect(DISTINCT {
        source: c.id,
        target: target.id,
        request: r.request,
        message_request: r.message_request
    }) AS all_calls
RETURN all_callables, all_calls
"#;

pub const DELETE_IMCG: &str = r#"
MATCH (c:Callable {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
DETACH DELETE c
"#;
