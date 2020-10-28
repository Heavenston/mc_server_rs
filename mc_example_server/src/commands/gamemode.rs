use mc_server_lib::chat_manager::CommandExecutor;
use mc_networking::data_types::command_data::LiteralNode;

use std::sync::Arc;
use mc_server_lib::entity::BoxedEntity;
use tokio::sync::RwLock;
use async_trait::async_trait;
use anyhow::Result;
use log::*;

pub struct GamemodeCommand;
#[async_trait]
impl CommandExecutor for GamemodeCommand {
    fn names(&self) -> Vec<String> {
        vec!["gamemode".to_string(), "gm".to_string()]
    }
    fn graph(&self) -> Vec<Arc<LiteralNode>> {
        let mut names = self.names().into_iter();
        let name = names.next().unwrap();
        let aliases: Vec<_> = names.collect();
        info!("Name: {:?}", name);
        info!("Aliases: {:?}", aliases);

        let mut main_node = LiteralNode {
            is_executable: false,
            children_nodes: vec![],
            redirect_node: None,
            name
        };

        main_node.children_nodes.push(Arc::new(LiteralNode {
            is_executable: true,
            children_nodes: vec![],
            redirect_node: None,
            name: "survival".to_string(),
        }));
        main_node.children_nodes.push(Arc::new(LiteralNode {
            is_executable: true,
            children_nodes: vec![],
            redirect_node: None,
            name: "creative".to_string(),
        }));
        main_node.children_nodes.push(Arc::new(LiteralNode {
            is_executable: true,
            children_nodes: vec![],
            redirect_node: None,
            name: "adventure".to_string(),
        }));

        let main_node = Arc::new(main_node);
        let mut nodes = vec![Arc::clone(&main_node)];
        for alias in aliases {
            nodes.push(Arc::new(LiteralNode {
                is_executable: false,
                children_nodes: vec![],
                redirect_node: None,
                name: alias
            }));
        }
        nodes
    }

    async fn on_command(&self, executor: Arc<RwLock<BoxedEntity>>, command: String, args: Vec<String>) -> Result<()> {
        if let BoxedEntity::Player(player) = &*executor.read().await {
            info!("{} executed {}", player.username, command);
        }
        Ok(())
    }
}
