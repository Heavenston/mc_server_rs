use crate::server::ENTITY_ID_COUNTER;
use mc_networking::data_types::command_data::{ArgumentNode, LiteralNode, Node};
use mc_server_lib::{
    chat_manager::CommandExecutor,
    entity::{living_entity::LivingEntity, BoxedEntity},
    entity_manager::PlayerWrapper,
    entity_pool::EntityPool,
    resource_manager::ResourceManager,
};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use tokio::sync::RwLock;

pub struct SummonCommand {
    pub entity_pool: Arc<EntityPool>,
    pub resource_manager: Arc<ResourceManager>,
}
#[async_trait]
impl CommandExecutor for SummonCommand {
    fn names(&self) -> Vec<String> {
        vec!["summon".to_string()]
    }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        vec![Arc::new(LiteralNode {
            is_executable: false,
            children_nodes: vec![Arc::new(LiteralNode {
                is_executable: false,
                children_nodes: vec![Arc::new(ArgumentNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "type".to_string(),
                    parser: "minecraft:resource_location".to_string(),
                    properties: vec![],
                    suggestions_type: None,
                })],
                redirect_node: None,
                name: "living".to_string(),
            })],
            redirect_node: None,
            name: self.names().first().unwrap().clone(),
        }) as Arc<dyn Node>]
    }

    async fn on_command(
        &self,
        executor: Arc<RwLock<BoxedEntity>>,
        _command: String,
        args: Vec<String>,
    ) -> Result<bool> {
        if let Some(player) = PlayerWrapper::new(executor).await {
            if args.len() < 1 {
                return Ok(false);
            }
            match &*args[0] {
                "living" => {
                    if args.len() != 2 {
                        Ok(false)
                    }
                    else {
                        let entity_type_name = &args[1];
                        let entity_type_name = if entity_type_name.contains(":") {
                            entity_type_name.clone()
                        }
                        else {
                            "minecraft:".to_string() + entity_type_name
                        };
                        match self
                            .resource_manager
                            .get_registry("minecraft:entity_type", Some(&entity_type_name))
                            .await
                        {
                            Some(id) => {
                                let mut entity = LivingEntity::new(
                                    ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
                                    uuid::Uuid::new_v4(),
                                    id,
                                );
                                entity.location = player.read().await.location().clone();
                                entity.on_ground = player.read().await.on_ground();
                                let entity = BoxedEntity::new(entity);
                                let entity = Arc::new(RwLock::new(entity));
                                self.entity_pool
                                    .entities
                                    .write()
                                    .await
                                    .add_entity(entity)
                                    .await;
                            }
                            None => player
                                .send_message(json!({
                                    "text": "Invalid entity type",
                                    "color": "red",
                                    "bold": "true"
                                }))
                                .await
                                .unwrap(),
                        }
                        Ok(true)
                    }
                }
                _ => Ok(false),
            }
        }
        else {
            Ok(false)
        }
    }
}
