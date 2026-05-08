use std::collections::HashMap;

use crate::constants::*;
use crate::voxel::Voxel;

struct Player {
    health: u32,
}

//Chunkmanager
struct ChunkManager {
    chunks: HashMap<ChunkCoord, Chunk>,
}

struct World {
    chunkmanager: ChunkManager,
}

pub struct GameState {
    // player: Player,
    world: World,
}
