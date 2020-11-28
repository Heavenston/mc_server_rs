use mc_networking::data_types::command_data::{LiteralNode, Node};
use mc_server_lib::{chat_manager::CommandExecutor, entity::player::PlayerRef};

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::entity::BoxedEntity;
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
            let player_ref = PlayerRef::new(executor).await.unwrap();
            match target_gamemode {
                "survival" => {
                    let mut player_entity = player_ref.entity.write().await;
                    let player_entity = player_entity.as_player_mut();
                    player_entity.can_fly = false;
                    player_entity.is_flying = false;
                    player_entity.invulnerable = false;
                    player_entity.gamemode = 0;
                }
                "creative" => {
                    let mut player_entity = player_ref.entity.write().await;
                    let player_entity = player_entity.as_player_mut();
                    player_entity.can_fly = true;
                    player_entity.invulnerable = true;
                    player_entity.gamemode = 1;
                }
                "adventure" => {
                    let mut player_entity = player_ref.entity.write().await;
                    let player_entity = player_entity.as_player_mut();
                    player_entity.can_fly = false;
                    player_entity.is_flying = false;
                    player_entity.invulnerable = false;
                    player_entity.gamemode = 2;
                }
                "spectator" => {
                    let mut player_entity = player_ref.entity.write().await;
                    let player_entity = player_entity.as_player_mut();
                    player_entity.can_fly = true;
                    player_entity.invulnerable = true;
                    player_entity.gamemode = 3;
                }
                _ => return Ok(false),
            }
            player_ref.update_gamemode().await;
            player_ref.update_abilities().await.unwrap();
            Ok(true)
        }
        else {
            Ok(false)
        }
    }
}
