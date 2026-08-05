pub const CREATE_SDG: &str = r#"
CALL {
    MATCH (s:Service {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
    DETACH DELETE s
}

UNWIND $services AS service
MERGE (s:Service {id: service.name, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
SET s.name = service.name
SET s.endpoints = service.endpoints
SET s.urls = service.urls

WITH ($connections + $message_connections) AS connections
UNWIND connections AS conn
MATCH (src:Service {id: conn.source_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MATCH (tgt:Service {id: conn.target_id, codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash}]->(tgt)
SET r.requests = coalesce(conn.requests, r.requests)
SET r.messages = coalesce(conn.messages, r.messages)
"#;

pub const GET_SDG: &str = r#"
MATCH (s:Service {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
WITH collect(DISTINCT s) AS all_services
OPTIONAL MATCH (source:Service)-[r:DEPENDS_ON]->(target:Service)
WHERE source.codebase_uuid = $codebase_uuid
  AND source.commit_hash = $commit_hash
  AND target.codebase_uuid = $codebase_uuid
  AND target.commit_hash = $commit_hash
WITH
    all_services,
    collect(DISTINCT CASE WHEN r.requests IS NOT NULL
        THEN {source: source.id, target: target.id, requests: r.requests}
    END) AS connection_candidates,
    collect(DISTINCT CASE WHEN r.messages IS NOT NULL
        THEN {source: source.id, target: target.id, messages: r.messages}
    END) AS message_connection_candidates
RETURN
    all_services,
    [connection IN connection_candidates WHERE connection IS NOT NULL] AS all_connections,
    [connection IN message_connection_candidates WHERE connection IS NOT NULL] AS all_message_connections
"#;

pub const DELETE_SDG: &str = r#"
MATCH (s:Service {codebase_uuid: $codebase_uuid, commit_hash: $commit_hash})
DETACH DELETE s
"#;
