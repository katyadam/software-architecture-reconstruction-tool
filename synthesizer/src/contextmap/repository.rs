use std::sync::Arc;

use actix_web::web;
use log::warn;
use models::Entity;
use neo4rs::{BoltType, Error, Graph, query};

use crate::contextmap::{
    model::{ContextMap, Dependency},
    queries::{CREATE_CONTEXT_MAP, DELETE_CONTEXT_MAP, GET_CONTEXT_MAP},
};

pub async fn save_context_map(
    graph: web::Data<Arc<Graph>>,
    context_map: &ContextMap,
    codebase_uuid: String,
) -> Result<String, Error> {
    match graph
        .run(
            query(CREATE_CONTEXT_MAP)
                .param("entities", context_map.entities.clone())
                .param("dependencies", context_map.dependencies.clone())
                .param("codebase_uuid", codebase_uuid),
        )
        .await
    {
        Ok(_) => Ok("Context map saved.".to_string()),
        Err(e) => {
            eprintln!("Failed to save context map: {:?}", e);
            Err(e)
        }
    }
}

pub async fn get_context_map(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid: String,
) -> Result<ContextMap, Error> {
    let mut result = graph
        .execute(query(GET_CONTEXT_MAP).param("codebase_uuid", codebase_uuid))
        .await?;
    let mut context_map = ContextMap {
        entities: Vec::new(),
        dependencies: Vec::new(),
    };

    if let Some(record) = result.next().await? {
        let entities_bolt_type: Vec<neo4rs::BoltType> = match record.get("all_entities") {
            Ok(value) => value,
            Err(e) => return Err(neo4rs::Error::DeserializationError(e)),
        };
        let dependencies_bolt_type: Vec<neo4rs::BoltType> = match record.get("all_dependencies") {
            Ok(value) => value,
            Err(e) => return Err(neo4rs::Error::DeserializationError(e)),
        };

        for bolt_entity in entities_bolt_type {
            if let BoltType::Node(node) = bolt_entity {
                match Entity::try_from(node) {
                    Ok(entity) => context_map.entities.push(entity),
                    Err(e) => warn!("Failed to deserialize Entity: {:?}", e),
                }
            } else {
                warn!("Unexpected BoltType for entity: {:?}", bolt_entity);
            }
        }

        for bolt_dep in dependencies_bolt_type {
            if let BoltType::Map(map) = bolt_dep {
                match Dependency::try_from(map) {
                    Ok(dep) => context_map.dependencies.push(dep),
                    Err(e) => warn!("Failed to deserialize Dependency: {:?}", e),
                }
            } else {
                warn!("Unexpected BoltType for dependency: {:?}", bolt_dep);
            }
        }
    }
    Ok(context_map)
}

pub async fn delete_context_map(
    graph: web::Data<Arc<Graph>>,
    codebase_uuid: String,
) -> Result<(), Error> {
    graph
        .run(query(DELETE_CONTEXT_MAP).param("codebase_uuid", codebase_uuid))
        .await
}
