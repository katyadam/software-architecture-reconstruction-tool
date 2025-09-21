use log::{error, info};
use neo4rs::{Graph, query};
use std::{env, sync::Arc};

async fn run_index_query(graph: &Arc<Graph>, index_query: &str) {
    match graph.run(query(index_query)).await {
        Ok(_) => info!("Successfully created index: {}", index_query),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already exists") {
                info!("Index already exists. Skipping: {}", index_query);
            } else {
                error!("Database setup error: {}", msg);
            }
        }
    };
}

async fn setup_graph(env_prefix: &str, index_query: &str) -> Arc<Graph> {
    let uri = env::var(format!("{env_prefix}_NEO4J_URI"))
        .unwrap_or_else(|_| panic!("{env_prefix}_NEO4J_URI must be set"));
    let user = env::var(format!("{env_prefix}_NEO4J_USERNAME"))
        .unwrap_or_else(|_| panic!("{env_prefix}_NEO4J_USERNAME must be set"));
    let pass = env::var(format!("{env_prefix}_NEO4J_PASSWORD"))
        .unwrap_or_else(|_| panic!("{env_prefix}_NEO4J_PASSWORD must be set"));

    let graph = Arc::new(Graph::new(uri, user, pass).await.unwrap());

    run_index_query(&graph, index_query).await;

    info!("Database setup complete for {env_prefix}");
    graph
}

pub async fn setup_contextmap_db() -> Arc<Graph> {
    setup_graph(
        "CM",
        "CREATE INDEX entity_id_codebase_uuid FOR (e:Entity) ON (e.id, e.codebase_uuid);",
    )
    .await
}

pub async fn setup_sdg_db() -> Arc<Graph> {
    setup_graph(
        "SDG",
        "CREATE INDEX service_id_codebase_uuid FOR (s:Service) ON (s.id, s.codebase_uuid);",
    )
    .await
}

pub async fn setup_imcg_db() -> Arc<Graph> {
    setup_graph(
        "IMCG",
        "CREATE INDEX callable_id_codebase_uuid FOR (c:Callable) ON (c.id, c.codebase_uuid);",
    )
    .await
}
