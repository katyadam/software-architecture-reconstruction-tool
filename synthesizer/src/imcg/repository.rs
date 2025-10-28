use std::sync::Arc;

use neo4rs::Graph;
use uuid::Uuid;

use crate::{errors::database::DatabaseError, imcg::model::IMCG};

pub trait ImcgRepository {
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, DatabaseError>;
    async fn save(&self, imcg: &IMCG, codebase_uuid: Uuid) -> Result<(), DatabaseError>;
    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), DatabaseError>;
}

pub struct ImcgRepositoryImpl {
    graph_handle: Arc<Graph>,
}

impl ImcgRepositoryImpl {
    pub fn new(graph_handle: Arc<Graph>) -> Self {
        Self { graph_handle }
    }
}

impl ImcgRepository for ImcgRepositoryImpl {
    async fn get_single(&self, codebase_uuid: Uuid) -> Result<IMCG, DatabaseError> {
        todo!();
    }

    async fn save(&self, imcg: &IMCG, codebase_uuid: Uuid) -> Result<(), DatabaseError> {
        todo!();
    }

    async fn delete(&self, codebase_uuid: Uuid) -> Result<(), DatabaseError> {
        todo!();
    }
}
