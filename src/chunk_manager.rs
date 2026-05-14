use std::collections::HashMap;

use crate::{
    chunk::{Chunk, ChunkCoord},
    chunk_rendering::GpuChunk,
    chunkmesher::MeshData,
    constants::RENDER_DISTANCE,
};

pub enum ChunkState {
    // Terrain data generated, no mesh yet
    Generated(Chunk),

    // Mesh computed, waiting for GPU upload
    // Happens for chunks just outside render distance,
    // or chunks inside render distance not yet uploaded
    Meshed(Chunk, MeshData),

    // Fully resident on GPU, ready to render
    Resident(Chunk, GpuChunk),
}

pub struct ChunkManager {
    chunks: HashMap<ChunkCoord, ChunkState>,
}

impl ChunkManager {
    pub fn new() -> Self {
        let chunks: HashMap<ChunkCoord, ChunkState> = HashMap::default();
        return Self { chunks };
    }

    /// Return neighboring chunks in the order expected by the chunkmesher
    /// PosX, NegX, PosY, NegY, PosZ, NegZ
    /// None if the chunk has not been loaded yet
    pub fn neighbors(&self, chunk_coord: ChunkCoord) -> [Option<&ChunkState>; 6] {
        let mut neighbors: [Option<&ChunkState>; 6] = [None; 6];

        let pos_x_neighbor = (chunk_coord.0 + 1, chunk_coord.1, chunk_coord.2);
        neighbors[0] = self.chunks.get(&pos_x_neighbor);
        let neg_x_neighbor = (chunk_coord.0 - 1, chunk_coord.1, chunk_coord.2);
        neighbors[1] = self.chunks.get(&neg_x_neighbor);

        let pos_y_neighbor = (chunk_coord.0, chunk_coord.1 + 1, chunk_coord.2);
        neighbors[2] = self.chunks.get(&pos_y_neighbor);
        let neg_y_neighbor = (chunk_coord.0, chunk_coord.1 - 1, chunk_coord.2);
        neighbors[3] = self.chunks.get(&neg_y_neighbor);

        let pos_z_neighbor = (chunk_coord.0, chunk_coord.1, chunk_coord.2 + 1);
        neighbors[4] = self.chunks.get(&pos_z_neighbor);
        let neg_z_neighbor = (chunk_coord.0 + 1, chunk_coord.1, chunk_coord.2 - 1);
        neighbors[5] = self.chunks.get(&neg_z_neighbor);

        neighbors
    }

    pub fn get_chunk(&self, chunk_coord: ChunkCoord) -> Option<&ChunkState> {
        self.chunks.get(&chunk_coord)
    }

    pub fn put_chunk(&mut self, chunk_coord: ChunkCoord, chunk: Chunk) {
        self.chunks
            .insert(chunk_coord, ChunkState::Generated(chunk));
    }

    pub fn update(&mut self, player_coord: ChunkCoord /* vulkan args */) {
        for (coord, state) in &mut self.chunks {
            match state {
                ChunkState::Generated(chunk) => {
                    // Needs meshing — kick off to worker thread
                }
                ChunkState::Meshed(chunk, mesh) => {
                    if within_render_distance(*coord, player_coord, RENDER_DISTANCE) {
                        // Needs GPU upload
                    }
                }
                ChunkState::Resident(chunk, gpu_chunk) => {
                    if !within_render_distance(*coord, player_coord, RENDER_DISTANCE) {
                        // Has moved out of render distance — free GPU memory,
                        // transition back to Meshed to keep mesh data ready
                    }
                }
            }
        }
    }

    // TODO rewrite this function
    // pub fn mark_dirty(&mut self, coord: ChunkCoord) {
    //     if let Some(state) = self.chunks.get_mut(&coord) {
    //         // Take ownership of the state to transition it
    //         let old = std::mem::replace(state, ChunkState::Generated(Chunk::empty()));
    //         *state = match old {
    //             ChunkState::Generated(c) => ChunkState::Generated(c),
    //             ChunkState::Meshed(c, _) => ChunkState::Generated(c), // discard stale mesh
    //             ChunkState::Resident(c, gpu) => {
    //                 // Free GPU memory before dropping GpuChunk
    //                 gpu.destroy(device);
    //                 ChunkState::Generated(c)
    //             }
    //         };
    //     }
    // }
}

fn within_render_distance(
    coord: ChunkCoord,
    player_coord: ChunkCoord,
    render_distance: usize,
) -> bool {
    true
}
