use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;
use mc_server_lib::entity_manager::PlayerWrapper;
use mc_server_lib::chunk_holder::ChunkHolder;
use mc_server_lib::resource_manager::ResourceManager;
use crate::generator::Generator;

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::entity::BoxedEntity;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RegenCommand {
    pub chunk_holder: Arc<ChunkHolder<Generator>>,
    pub resource_manager: Arc<ResourceManager>,
}
#[async_trait]
impl CommandExecutor for RegenCommand {
    fn names(&self) -> Vec<String> { vec!["regen".to_string()] }
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
            let location = player.read().await.location().clone();
            self.chunk_holder.generate_chunk(
                location.chunk_x(), location.chunk_z(),
                Generator::new(false, self.resource_manager.clone())
            ).await;
            Ok(true)
        }
        else {
            Ok(false)
        }
    }
}
