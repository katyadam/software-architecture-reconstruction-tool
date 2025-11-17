pub const CREATE_SDG: &str = r#"
UNWIND $services AS service
MERGE (s:Service {id: service.name, codebase_uuid: $codebase_uuid})
SET s.name = service.name
SET s.endpoints = service.endpoints
SET s.urls = service.urls
SET s.codebase_uuid = $codebase_uuid

WITH $connections AS connections
UNWIND connections AS conn
MATCH (src:Service {id: conn.source_id, codebase_uuid: $codebase_uuid})
MATCH (tgt:Service {id: conn.target_id, codebase_uuid: $codebase_uuid})
MERGE (src)-[r:DEPENDS_ON {codebase_uuid: $codebase_uuid}]->(tgt)
SET r.requests = conn.requests 
"#;

pub const GET_SDG: &str = r#"
MATCH (s:Service {codebase_uuid: $codebase_uuid})
OPTIONAL MATCH (s)-[r:DEPENDS_ON]->(target:Service)
WHERE target.codebase_uuid = $codebase_uuid
WITH
    collect(DISTINCT s) AS all_services,
    collect(DISTINCT {source: s.id, target: target.id, requests: r.requests}) AS all_connections
RETURN all_services, all_connections
"#;

pub const DELETE_SDG: &str = r#"
MATCH (s:Service {codebase_uuid: $codebase_uuid})
DETACH DELETE s
"#;
