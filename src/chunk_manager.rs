use std::{collections::HashMap, mem};

use png::chunk;

use crate::{
    app::RenderApp,
    chunk::{Chunk, ChunkCoord},
    chunk_rendering::GpuChunk,
    chunkmesher::MeshData,
    constants::RENDER_DISTANCE,
    instance,
    terrain_generator::TerrainGenerator,
    utils::*,
};

pub enum ChunkState {
    // Not generated yet
    Unloaded,
    // Terrain data generated or loaded from disk
    Generated(Chunk),

    // Empty chunks are never meshed or uploaded
    Empty(Chunk),

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
    pub fn neighbors(&self, chunk_coord: &ChunkCoord) -> [Option<&ChunkState>; 6] {
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

    pub fn queue_chunk(&mut self, chunk_coord: ChunkCoord) {
        println!("Inserting chunk at {:?}", chunk_coord);
        self.chunks.insert(chunk_coord, ChunkState::Unloaded);
    }

    /// Updates Chunks to based on their current state
    /// Queues meshing and GPU uploading when necesssary
    pub fn update(
        &mut self,
        player_coord: ChunkCoord, /* vulkan args */
        terrain_generator: &TerrainGenerator,
        render_app: &mut RenderApp,
    ) -> Result<()> {
        for (coord, state) in &mut self.chunks {
            match state {
                ChunkState::Unloaded => {
                    debug!("Generating chunk: {:?}", coord);
                    // Chunks need to be loaded
                    let chunk = terrain_generator.generate_chunk(coord);

                    if chunk.is_empty() {
                        debug!("chunk {:?} is empty", coord);
                        *state = ChunkState::Empty(chunk);
                    } else {
                        debug!("chunk {:?} generated", coord);
                        *state = ChunkState::Generated(chunk);
                    }
                }
                ChunkState::Empty(_) => continue, // Todo: add drop when out of loading distance.

                ChunkState::Generated(_) => {
                    // Needs meshing
                    // TODO kick off to worker thread
                    // Move the old state out
                    debug!("Meshing chunk {:?}", coord);
                    let old_state = mem::replace(state, ChunkState::Unloaded);

                    if let ChunkState::Generated(chunk) = old_state {
                        let mesh_data = chunk.mesh([None; 6]);
                        debug!("Meshing succesful: {:?}", coord);
                        *state = ChunkState::Meshed(chunk, mesh_data);
                    }
                }
                ChunkState::Meshed(_, _) => {
                    if within_render_distance(*coord, player_coord, RENDER_DISTANCE) {
                        // Needs GPU upload

                        let old_state = mem::replace(state, ChunkState::Unloaded);

                        if let ChunkState::Meshed(chunk, mesh_data) = old_state {
                            unsafe {
                                let gpu_chunk = GpuChunk::new(
                                    &render_app.instance,
                                    &render_app.device,
                                    render_app.data.physical_device,
                                    render_app.data.graphics_queue,
                                    render_app.data.command_pool,
                                    &mesh_data,
                                    *coord,
                                )?;
                                debug!("Chunk uploaded to GPU: {:?}", coord);
                                *state = ChunkState::Resident(chunk, gpu_chunk)
                            }
                        }
                    }
                }
                ChunkState::Resident(chunk, gpu_chunk) => {
                    if !within_render_distance(*coord, player_coord, RENDER_DISTANCE) {
                        // Has moved out of render distance — free GPU memory,
                        // transition back to Meshed to keep mesh data ready
                        continue;

                        // (call destroy for the chunks)
                    }
                }
            }
        }

        return Ok(());
    }

    /// Returns an iterator over the currently visible chunks
    /// TODO add Frustum Culling
    pub fn visible_chunks(&self) -> impl Iterator<Item = &GpuChunk> {
        self.chunks.values().filter_map(|state| match state {
            ChunkState::Resident(_, gpu_chunk) => Some(gpu_chunk),
            _ => None,
        })
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

    pub unsafe fn destroy(&mut self, instance: &Instance, device: &Device) {
        for (coord, state) in &mut self.chunks {
            match state {
                ChunkState::Resident(_, gpu_chunk) => {
                    *&gpu_chunk.destroy(instance, device);
                }
                _ => {}
            }
        }
    }
}

fn within_render_distance(
    coord: ChunkCoord,
    player_coord: ChunkCoord,
    render_distance: usize,
) -> bool {
    true
}
