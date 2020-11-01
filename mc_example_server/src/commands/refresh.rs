use crate::generator::Generator;
use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::{
    chat_manager::CommandExecutor, chunk_holder::ChunkHolder, entity_manager::PlayerWrapper,
    resource_manager::ResourceManager,
};

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::entity::BoxedEntity;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RefreshCommand {
    pub chunk_holder: Arc<ChunkHolder<Generator>>,
    pub resource_manager: Arc<ResourceManager>,
}
#[async_trait]
impl CommandExecutor for RefreshCommand {
    fn names(&self) -> Vec<String> {
        vec!["refresh".to_string()]
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
        executor: Arc<RwLock<BoxedEntity>>,
        _command: String,
        _args: Vec<String>,
    ) -> Result<bool> {
        if let Some(player) = PlayerWrapper::new(executor).await {
            let id = player.read().await.entity_id();
            self.chunk_holder.refresh_player_chunks(id).await;
            Ok(true)
        }
        else {
            Ok(false)
        }
    }
}
