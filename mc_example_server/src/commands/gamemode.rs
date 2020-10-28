use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;

use anyhow::Result;
use async_trait::async_trait;
use log::*;
use mc_server_lib::entity::BoxedEntity;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct GamemodeCommand;
#[async_trait]
impl CommandExecutor for GamemodeCommand {
    fn names(&self) -> Vec<String> { vec!["gamemode".to_string(), "gm".to_string()] }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        let mut names = self.names().into_iter();
        let name = names.next().unwrap();
        let aliases: Vec<_> = names.collect();
        info!("Name: {:?}", name);
        info!("Aliases: {:?}", aliases);

        let main_node = Arc::new(LiteralNode {
            is_executable: false,
            children_nodes: vec![
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "survival".to_string(),
                }),
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "creative".to_string(),
                }),
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "adventure".to_string(),
                }),
            ],
            redirect_node: None,
            name,
        }) as Arc<dyn Node>;
        let mut nodes = vec![Arc::clone(&main_node)];
        for alias in aliases {
            nodes.push(Arc::new(LiteralNode {
                is_executable: false,
                children_nodes: vec![],
                redirect_node: Some(Arc::clone(&main_node)),
                name: alias
            }) as Arc<dyn Node>);
        }
        nodes
    }

    async fn on_command(
        &self,
        executor: Arc<RwLock<BoxedEntity>>,
        command: String,
        args: Vec<String>,
    ) -> Result<()> {
        if let BoxedEntity::Player(player) = &*executor.read().await {
            info!("{} executed /{} {:?}", player.username, command, args);
        }
        Ok(())
    }
}
