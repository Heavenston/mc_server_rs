use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::{entity::BoxedEntity, entity_manager::PlayerWrapper};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct GamemodeCommand;
#[async_trait]
impl CommandExecutor for GamemodeCommand {
    fn names(&self) -> Vec<String> {
        vec!["gamemode".to_string(), "gm".to_string()]
    }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        let mut names = self.names().into_iter();
        let name = names.next().unwrap();
        let aliases: Vec<_> = names.collect();

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
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "spectator".to_string(),
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
                name: alias,
            }) as Arc<dyn Node>);
        }
        nodes
    }

    async fn on_command(
        &self,
        executor: Arc<RwLock<BoxedEntity>>,
        _command: String,
        args: Vec<String>,
    ) -> Result<bool> {
        if args.len() != 1 {
            return Ok(false);
        }
        let target_gamemode = args[0].as_str();
        let is_player = executor.read().await.is_player();
        if is_player {
            let player = PlayerWrapper::from(executor);
            match target_gamemode {
                "survival" => {
                    player.set_gamemode(0).await;
                    Ok(true)
                }
                "creative" => {
                    player.set_gamemode(1).await;
                    Ok(true)
                }
                "adventure" => {
                    player.set_gamemode(2).await;
                    Ok(true)
                }
                "spectator" => {
                    player.set_gamemode(3).await;
                    Ok(true)
                }
                _ => Ok(false),
            }
        }
        else {
            Ok(false)
        }
    }
}
