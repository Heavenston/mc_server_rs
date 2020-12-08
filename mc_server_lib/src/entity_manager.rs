use crate::entity::{
    player::{PlayerEntity, PlayerRef},
    BoxedEntity,
};
use mc_networking::packets::{client_bound::ClientBoundPacket, RawPacket};

use anyhow::{Error, Result};
use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use std::{ops::Index, sync::Arc};
use tokio::sync::RwLock;

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

#[derive(Clone)]
pub struct EntityManager<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> {
    entities: FxIndexMap<i32, T>,
}

pub type PlayerManager = EntityManager<PlayerRef>;
pub type BoxedEntityManager = EntityManager<Arc<RwLock<BoxedEntity>>>;

impl<T: Into<Arc<RwLock<BoxedEntity>>> + Clone> EntityManager<T> {
    pub fn new() -> Self {
        Self {
            entities: Default::default(),
        }
    }

    pub fn size(&self) -> usize {
        self.entities.len()
    }

    pub fn get_entities(&self) -> &FxIndexMap<i32, T> {
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
    pub async fn broadcast(&self, packet: &impl ClientBoundPacket) {
        for entity in self.entities() {
            entity.send_packet_async(packet).await;
        }
    }
    pub async fn broadcast_to(packet: &impl ClientBoundPacket, players: &Vec<PlayerRef>) {
        for player in players {
            player.send_packet_async(packet).await;
        }
    }
    pub async fn send_to_player(
        &self,
        player_id: i32,
        packet: &impl ClientBoundPacket,
    ) -> Result<()> {
        self.get_entity(player_id)
            .ok_or(Error::msg("Invalid player id"))?
            .send_packet_async(packet)
            .await;
        Ok(())
    }
    pub async fn send_raw_to_player(&self, player_id: i32, packet: RawPacket) -> Result<()> {
        self.get_entity(player_id)
            .ok_or(Error::msg("Invalid player id"))?
            .send_raw_packet_async(packet)
            .await;
        Ok(())
    }
    pub async fn get_filtered_players(
        &self,
        filter: impl Fn(&PlayerEntity) -> bool,
    ) -> Vec<PlayerRef> {
        let mut players = Vec::new();
        for player_ref in self.entities() {
            let result = {
                let player = player_ref.entity.read().await;
                let player = player.as_player().as_ref();
                filter(player)
            };
            if result {
                players.push(player_ref.clone());
            }
        }
        players
    }
    pub async fn get_players_except(&self, except_id: i32) -> Vec<PlayerRef> {
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
    type IntoIter = indexmap::map::IntoIter<i32, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.into_iter()
    }
}

impl<'a, T: Into<Arc<RwLock<BoxedEntity>>> + Clone> IntoIterator for &'a EntityManager<T> {
    type Item = (&'a i32, &'a T);
    type IntoIter = indexmap::map::Iter<'a, i32, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entities.iter()
    }
}
