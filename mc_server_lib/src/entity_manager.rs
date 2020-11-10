use crate::entity::{player::Player, BoxedEntity};
use mc_networking::packets::{client_bound::*, RawPacket};

use anyhow::{Error, Result};
use std::{
    collections::HashMap,
    ops::{Deref, Index},
    sync::Arc,
};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct PlayerWrapper {
    pub entity: Arc<RwLock<BoxedEntity>>,
}
impl PlayerWrapper {
    pub async fn new(entity: Arc<RwLock<BoxedEntity>>) -> Option<Self> {
        if !entity.read().await.is_player() {
            return None;
        }
        Some(Self { entity })
    }

    pub async fn send_packet(&self, packet: &impl ClientBoundPacket) -> Result<()> {
        self.read()
            .await
            .as_player()
            .client
            .read()
            .await
            .send_packet(packet)
            .await?;
        Ok(())
    }
    pub async fn send_message(&self, message: serde_json::Value) -> Result<()> {
        self.send_packet(&C0EChatMessage {
            json_data: message,
            position: 0,
            sender: None,
        })
        .await
    }
    pub async fn entity_id(&self) -> i32 {
        self.entity.read().await.entity_id()
    }
    pub async fn set_gamemode(&self, gm: u8) {
        {
            let mut player = self.entity.write().await;
            let player = player.as_player_mut();
            player.gamemode = gm;
            match gm {
                0 => {
                    // Survival
                    player.can_fly = false;
                    player.is_flying = false;
                    player.invulnerable = false;
                }
                1 => {
                    // Creative
                    player.can_fly = true;
                    player.invulnerable = true;
                }
                2 => {
                    // Adventure
                    player.can_fly = false;
                    player.is_flying = false;
                    player.invulnerable = false;
                }
                3 => {
                    // Spectator
                    player.can_fly = true;
                    player.invulnerable = true;
                }
                _ => unimplemented!(),
            }
        }
        self.send_packet(&C1DChangeGameState {
            reason: 3, // Change Gamemode
            value: gm as f32,
        })
        .await
        .unwrap();
        self.update_abilities().await.unwrap();
    }

    pub async fn update_abilities(&self) -> Result<()> {
        let player = self.entity.read().await;
        let player = player.as_player();
        player
            .client
            .read()
            .await
            .send_player_abilities(
                player.invulnerable,
                player.is_flying,
                player.can_fly,
                player.gamemode == 1,
                player.flying_speed,
                player.fov_modifier,
            )
            .await?;
        Ok(())
    }
}
impl Deref for PlayerWrapper {
    type Target = Arc<RwLock<BoxedEntity>>;

    fn deref(&self) -> &Self::Target {
        &self.entity
    }
}
impl Into<Arc<RwLock<BoxedEntity>>> for PlayerWrapper {
    fn into(self) -> Arc<RwLock<BoxedEntity>> {
        self.entity
    }
}
impl From<Arc<RwLock<BoxedEntity>>> for PlayerWrapper {
    fn from(entity: Arc<RwLock<BoxedEntity>>) -> Self {
        Self { entity }
    }
}

#[derive(Clone)]
pub struct EntityManager<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> {
    entities: HashMap<i32, T>,
}

pub type PlayerManager = EntityManager<PlayerWrapper>;
pub type BoxedEntityManager = EntityManager<Arc<RwLock<BoxedEntity>>>;

impl<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> EntityManager<T> {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    pub fn size(&self) -> usize {
        self.entities.len()
    }

    pub fn get_entities(&self) -> &HashMap<i32, T> {
        &self.entities
    }
    pub fn has_entity(&self, entity_id: i32) -> bool {
        self.entities.contains_key(&entity_id)
    }
    pub fn get_entity(&self, entity_id: i32) -> Option<&T> {
        self.entities.get(&entity_id)
    }
    pub async fn add_entity(&mut self, entity: impl Into<T>) {
        let entity = entity.into();
        let entity_id = entity.clone().into().read().await.entity_id();
        self.entities.insert(entity_id, entity);
    }
    pub fn remove_entity(&mut self, entity_id: i32) -> Option<T> {
        self.entities.remove(&entity_id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &T> {
        self.entities.values()
    }
    pub fn ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.entities.keys().cloned()
    }
    pub fn iter(&self) -> impl Iterator<Item = (i32, &T)> {
        self.entities.iter().map(|(k, v)| (*k, v))
    }
}

impl PlayerManager {
    pub async fn broadcast(&self, packet: &impl ClientBoundPacket) -> Result<()> {
        for entity in self.entities() {
            entity.send_packet(packet).await?;
        }
        Ok(())
    }
    pub async fn broadcast_to(packet: &impl ClientBoundPacket, players: Vec<PlayerWrapper>) {
        for player in players {
            player.send_packet(packet).await.unwrap();
        }
    }
    pub async fn send_to_player(
        &self,
        player_id: i32,
        packet: &impl ClientBoundPacket,
    ) -> Result<()> {
        self.get_entity(player_id)
            .ok_or(Error::msg("Invalid player id"))?
            .send_packet(packet)
            .await?;
        Ok(())
    }
    pub async fn send_raw_to_player(&self, player_id: i32, packet: &RawPacket) -> Result<()> {
        self.get_entity(player_id)
            .ok_or(Error::msg("Invalid player id"))?
            .read()
            .await
            .as_player()
            .client
            .read()
            .await
            .send_raw_packet(packet)
            .await?;
        Ok(())
    }
    pub async fn get_filtered_players(
        &self,
        filter: impl Fn(&Player) -> bool,
    ) -> Vec<PlayerWrapper> {
        let mut players = Vec::new();
        for player in self.entities() {
            let result = {
                let player = player.read().await;
                let player = player.as_player().as_ref();
                filter(player)
            };
            if result {
                players.push(player.clone());
            }
        }
        players
    }
    pub async fn get_players_except(&self, except_id: i32) -> Vec<PlayerWrapper> {
        self.iter()
            .filter(|(id, ..)| id != &except_id)
            .map(|(.., v)| v.clone())
            .collect::<Vec<_>>()
    }
}

impl<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> Index<i32> for EntityManager<T> {
    type Output = T;

    fn index(&self, index: i32) -> &Self::Output {
        self.get_entity(index).unwrap()
    }
}

impl<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> IntoIterator for EntityManager<T> {
    type Item = (i32, T);
    type IntoIter = std::collections::hash_map::IntoIter<i32, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_iter()
    }
}

impl<'a, T: Into<Arc<RwLock<BoxedEntity>>> + Clone> IntoIterator for &'a EntityManager<T> {
    type Item = (&'a i32, &'a T);
    type IntoIter = std::collections::hash_map::Iter<'a, i32, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.iter()
    }
}
