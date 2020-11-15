use mc_networking::data_types::command_data::{ArgumentNode, LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;

use anyhow::Result;
use async_trait::async_trait;
use mc_server_lib::{entity::BoxedEntity, entity_pool::EntityPool};
use mc_utils::Location;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct TpCommand {
    pub entity_pool: Arc<EntityPool>,
}
#[async_trait]
impl CommandExecutor for TpCommand {
    fn names(&self) -> Vec<String> {
        vec!["tp".to_string(), "teleport".to_string()]
    }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        let mut names = self.names().into_iter();
        let name = names.next().unwrap();
        let aliases: Vec<_> = names.collect();

        let main_node = Arc::new(LiteralNode {
            is_executable: false,
            children_nodes: vec![Arc::new(ArgumentNode {
                is_executable: true,
                children_nodes: vec![],
                redirect_node: None,
                name: "location".to_string(),
                parser: "minecraft:vec3".to_string(),
                properties: vec![],
                suggestions_type: None,
            })],
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
        if args.len() != 3 {
            return Ok(false);
        }
        let mut entity = executor.write().await;
        let location = entity.location();
        let parse_coord = |x: &str, default: f64| {
            if x == "~" {
                return Some(default);
            }
            match x.parse::<i32>() {
                Ok(block) => Some(block as f64 + if block < 0 { -0.5 } else { 0.5 }),
                Err(..) => match x.parse::<f64>() {
                    Ok(pos) => Some(pos),
                    Err(..) => None,
                },
            }
        };

        let x = match parse_coord(&args[0], location.x) {
            Some(n) => n,
            None => return Ok(false),
        };
        let y = match parse_coord(&args[1], location.y) {
            Some(n) => n,
            None => return Ok(false),
        };
        let z = match parse_coord(&args[2], location.z) {
            Some(n) => n,
            None => return Ok(false),
        };

        self.entity_pool
            .teleport_entity(
                &mut *entity,
                Location {
                    x,
                    y,
                    z,
                    yaw: 0.0,
                    pitch: 0.0,
                },
            )
            .await;

        Ok(true)
    }
}
