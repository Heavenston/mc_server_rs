use legion::Entity;

/// Trait to represent a chunk provider which manages chunk loading
pub trait ChunkProvider: Send + Sync {
    /// Called when a chunk gets in range of a player
    /// You may send the ChunkData packet at any time after this
    fn load_chunk(&self, player: &Entity, x: i32, z: i32);
    /// Called when a chunk leave the range of a player
    /// You should send a chunk unload packet and cancel any running chunk loading
    fn unload_chunk(&self, player: &Entity, x: i32, z: i32);
}
