use rand::seq::IndexedRandom;

use crate::{chunk::Chunk, constants::*, voxel::Voxel};

pub struct TerrainGenerator {
    biome_map: f32, // TODO: implement
}

impl TerrainGenerator {
    pub fn new() -> Self {
        let biome_map = 1.;
        return Self { biome_map };
    }

    pub fn generate_chunk(&self, coord: &(i32, i32, i32)) -> Chunk {
        // Temporary: create chunk meshdata to display.
        let red_voxel = Voxel::new(255, 0, 80);
        let green_voxel = Voxel::new(40, 255, 0);
        let blue_voxel = Voxel::new(0, 70, 255);

        const VOXEL_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
        let voxel_options = [red_voxel, green_voxel, blue_voxel];
        let voxels: [Voxel; VOXEL_COUNT] =
            std::array::from_fn(|_| voxel_options.choose(&mut rand::rng()).unwrap().clone());
        let active_voxels: [u64; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64] =
            // std::array::from_fn(|_| rand::random());
        [u64::MAX; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64];

        let chunk = Chunk::create(voxels, active_voxels, *coord);

        chunk
    }
}
