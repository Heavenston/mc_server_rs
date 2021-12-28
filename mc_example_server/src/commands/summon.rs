use crate::{entities::ghost::GhostEntity, server::ENTITY_ID_COUNTER};
use mc_networking::data_types::{
    command_data::{ArgumentNode, LiteralNode, Node},
    encoder::PacketEncoder,
};
use mc_server_lib::{
    chat_manager::CommandExecutor,
    entity::{living_entity::LivingEntity, player::PlayerRef, BoxedEntity},
    entity_pool::EntityPool,
    resource_manager::ResourceManager,
};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

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
        let amount_arg = Arc::new(ArgumentNode {
            is_executable: true,
            children_nodes: vec![],
            redirect_node: None,
            name: "amount".to_string(),
            parser: "brigadier:integer".into(),
            properties: {
                let mut encoder = PacketEncoder::default();
                encoder.write_u8(0x01);
                encoder.write_i32(1);
                encoder.into_inner().freeze()
            },
            suggestions_type: None,
        }) as Arc<dyn Node>;

        vec![Arc::new(LiteralNode {
            is_executable: false,
            children_nodes: vec![
                Arc::new(LiteralNode {
                    is_executable: false,
                    children_nodes: vec![Arc::new(ArgumentNode {
                        is_executable: true,
                        children_nodes: vec![Arc::clone(&amount_arg)],
                        redirect_node: None,
                        name: "type".to_string(),
                        parser: "minecraft:resource_location".into(),
                        properties: Bytes::new(),
                        suggestions_type: None,
                    })],
                    redirect_node: None,
                    name: "living".to_string(),
                }),
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![Arc::clone(&amount_arg)],
                    redirect_node: None,
                    name: "ghost".to_string(),
                }),
            ],
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
        if let Some(player_ref) = PlayerRef::new(executor).await {
            if args.is_empty() {
                return Ok(false);
            }
            match &*args[0] {
                "ghost" => {
                    let amount: i32 = if args.len() == 2 {
                        let parsed = args[1].parse();
                        if let Err(..) = parsed {
                            return Ok(false);
                        }
                        parsed.unwrap()
                    } else {
                        1
                    };
                    let player_location = player_ref.entity.read().await.location().clone();
                    let mut entities = self.entity_pool.entities.write().await;
                    for _ in 0..amount {
                        let eid = ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
                        let mut entity = GhostEntity::new(
                            eid,
                            Uuid::new_v4(),
                            Arc::downgrade(&player_ref.entity),
                        );
                        entity.location = player_location.clone();
                        entity.location.y += 1.5;
                        entity.on_ground = false;
                        let entity = BoxedEntity::new(entity);
                        let entity = Arc::new(RwLock::new(entity));
                        entities.add_entity(entity).await;
                    }
                    Ok(true)
                }
                "living" => {
                    if args.len() != 2 && args.len() != 3 {
                        Ok(false)
                    } else {
                        let entity_type_name = &args[1];
                        let entity_type_name = if entity_type_name.contains(':') {
                            entity_type_name.clone()
                        } else {
                            "minecraft:".to_string() + entity_type_name
                        };
                        match self
                            .resource_manager
                            .get_registry("minecraft:entity_type", Some(&entity_type_name))
                            .await
                        {
                            Some(id) => {
                                let amount: i32 = if args.len() == 3 {
                                    let parsed = args[2].parse();
                                    if let Err(..) = parsed {
                                        return Ok(false);
                                    }
                                    parsed.unwrap()
                                } else {
                                    1
                                };
                                let player_location =
                                    player_ref.entity.read().await.location().clone();
                                let on_ground = player_ref.entity.read().await.on_ground();
                                let mut entities = self.entity_pool.entities.write().await;
                                for _ in 0..amount {
                                    let mut entity = LivingEntity::new(
                                        ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
                                        uuid::Uuid::new_v4(),
                                        id,
                                    );
                                    entity.location = player_location.clone();
                                    entity.on_ground = on_ground;
                                    let entity = BoxedEntity::new(entity);
                                    let entity = Arc::new(RwLock::new(entity));
                                    entities.add_entity(entity).await;
                                }
                            }
                            None => {
                                player_ref
                                    .send_chat_message(json!({
                                        "text": "Invalid entity type",
                                        "color": "red",
                                        "bold": "true"
                                    }))
                                    .await
                            }
                        }
                        Ok(true)
                    }
                }
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }
}
