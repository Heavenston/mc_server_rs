use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::{
    chat_manager::CommandExecutor, entity_pool::EntityPool, resource_manager::ResourceManager,
};

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::entity::BoxedEntity;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AdiosCommand {
    pub entity_pool: Arc<EntityPool>,
    pub resource_manager: Arc<ResourceManager>,
}
#[async_trait]
impl CommandExecutor for AdiosCommand {
    fn names(&self) -> Vec<String> {
        vec!["adios".to_string()]
    }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        vec![Arc::new(LiteralNode {
            is_executable: true,
            children_nodes: vec![],
            redirect_node: None,
            name: self.names().first().unwrap().clone(),
        }) as Arc<dyn Node>]
    }

    async fn on_command(
        &self,
        _executor: Arc<RwLock<BoxedEntity>>,
        _command: String,
        _args: Vec<String>,
    ) -> Result<bool> {
        let ids = self
            .entity_pool
            .entities
            .read()
            .await
            .ids()
            .collect::<Vec<_>>();
        for id in ids {
            let entity = self
                .entity_pool
                .entities
                .read()
                .await
                .get_entity(id)
                .unwrap()
                .clone();
            if !entity.read().await.is_player() {
                self.entity_pool
                    .entities
                    .write()
                    .await
                    .remove_entity(id)
                    .unwrap();
            }
        }
        Ok(true)
    }
}
