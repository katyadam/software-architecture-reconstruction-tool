use std::sync::Arc;

use actix_web::web;
use log::warn;
use neo4rs::{BoltType, Error, Graph, query};

use crate::sdg::{
    model::{Connection, SDG, Service},
    queries::{CREATE_SDG, DELETE_SDG, GET_SDG},
};

pub async fn save_sdg(
    graph: web::Data<Arc<Graph>>,
    sdg: &SDG,
    codebase_uuid: String,
) -> Result<String, Error> {
    match graph
        .run(
            query(CREATE_SDG)
                .param("services", sdg.services.clone())
                .param("connections", sdg.connections.clone())
                .param("codebase_uuid", codebase_uuid),
        )
        .await
    {
        Ok(_) => Ok("SDG saved.".to_string()),
        Err(e) => {
            eprintln!("Failed to save SDG: {:?}", e);
            Err(e)
        }
    }
}

pub async fn get_sdg(graph: web::Data<Arc<Graph>>, codebase_uuid: String) -> Result<SDG, Error> {
    let mut result = graph
        .execute(query(GET_SDG).param("codebase_uuid", codebase_uuid))
        .await?;

    let mut sdg = SDG {
        services: Vec::new(),
        connections: Vec::new(),
    };

    if let Some(record) = result.next().await? {
        let services_bolt_type: Vec<neo4rs::BoltType> = match record.get("all_services") {
            Ok(value) => value,
            Err(e) => return Err(neo4rs::Error::DeserializationError(e)),
        };
        let connections_bolt_type: Vec<neo4rs::BoltType> = match record.get("all_connections") {
            Ok(value) => value,
            Err(e) => return Err(neo4rs::Error::DeserializationError(e)),
        };

        for bolt_service in services_bolt_type {
            if let BoltType::Node(node) = bolt_service {
                match Service::try_from(node) {
                    Ok(entity) => sdg.services.push(entity),
                    Err(e) => warn!("Failed to deserialize Entity: {:?}", e),
                }
            } else {
                warn!("Unexpected BoltType for entity: {:?}", bolt_service);
            }
        }

        for bolt_dep in connections_bolt_type {
            if let BoltType::Map(map) = bolt_dep {
                match Connection::try_from(map) {
                    Ok(dep) => sdg.connections.push(dep),
                    Err(e) => warn!("Failed to deserialize Connection: {:?}", e),
                }
            } else {
                warn!("Unexpected BoltType for connection: {:?}", bolt_dep);
            }
        }
    }
    Ok(sdg)
}

pub async fn delete_sdg(graph: web::Data<Arc<Graph>>, codebase_uuid: String) -> Result<(), Error> {
    graph
        .run(query(DELETE_SDG).param("codebase_uuid", codebase_uuid))
        .await
}
