use std::collections::HashMap;

use crate::constants::*;
use crate::voxel::Voxel;

pub type ChunkCoord = (i32, i32, i32);

pub struct Chunk {
    pub voxels: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub active_voxels: [u64; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64],
    pub position: ChunkCoord,
    pub is_dirty: bool,
    pub is_empty: bool,
}

impl Chunk {
    pub fn create(
        voxels: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        active_voxels: [u64; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE / 64],
        position: ChunkCoord,
    ) -> Self {
        let is_dirty = true;
        let is_empty = active_voxels.iter().all(|&word| word == 0);

        return Self {
            voxels,
            active_voxels,
            position,
            is_dirty,
            is_empty,
        };
    }

    pub fn is_empty(&self) -> bool {
        return self.active_voxels.iter().all(|&word| word == 0);
    }

    pub fn is_active(&self, idx: usize) -> bool {
        let word = idx / 64;
        let bit = idx % 64;
        (self.active_voxels[word] >> bit) & 1 == 1
    }

    pub fn set_active(&mut self, idx: usize, val: bool) {
        let word = idx / 64;
        let bit = idx % 64;
        if val {
            self.active_voxels[word] |= (1u64 << bit);
        } else {
            self.active_voxels[word] &= !(1u64 << bit);
        }
    }

    pub fn set(&mut self, idx: usize, voxel: Option<Voxel>) {
        match voxel {
            Some(v) => {
                self.voxels[idx] = v;
                self.set_active(idx, true);
            }
            None => {
                self.set_active(idx, false);
            }
        }
    }
}
