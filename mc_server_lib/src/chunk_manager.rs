use std::ops::Deref;
use bevy_ecs::entity::Entity;

/// A chunk provider is use to generate chunks and send them to players
pub trait ChunkProvider: Send + Sync {
    /// Called when a chunk gets in range of a player
    /// You may send the ChunkData packet at any time after this
    fn load_chunk(&mut self, player: Entity, x: i32, z: i32);
    /// Called when a chunk leave the range of a player
    /// You should send a chunk unload packet and cancel any running chunk loading
    fn unload_chunk(&mut self, player: Entity, x: i32, z: i32);
}

pub trait ConstChunkProvider: Send + Sync {
    fn const_load_chunk(&self, player: Entity, x: i32, z: i32);
    fn const_unload_chunk(&self, player: Entity, x: i32, z: i32);
}

impl<T, U> ChunkProvider for T 
    where T: Deref<Target = U> + Send + Sync,
          U: ConstChunkProvider
{
    fn load_chunk(&mut self, player: Entity, x: i32, z: i32) {
        self.const_load_chunk(player, x, z);
    }
    fn unload_chunk(&mut self, player: Entity, x: i32, z: i32){
        self.const_load_chunk(player, x, z);
    }
}
