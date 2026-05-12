use bytemuck::{Pod, Zeroable};

use crate::chunk::Chunk;
use crate::constants::*;
use crate::voxel::Voxel;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VoxelVertex {
    // 16 bytes atm. No need to keep this aligned to multiples of 2.
    // Vulkan can handle different offset
    pub pos: [f32; 3],  // 12 bytes
    pub normal: i8,     // 1 byte: only 6 possible options for a vertex
    pub color: [u8; 3], // 3 bytes RGB value
}

impl VoxelVertex {
    fn new(pos: [f32; 3], normal: i8, color: [u8; 3]) -> Self {
        Self { pos, normal, color }
    }
}

#[derive(Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

// The 6 face directions. Order matches the axis loops below.
// Each entry: (normal, u_axis, v_axis)
// u/v axes define which two coordinates span the face quad,
// so the mesher can iterate slices orthogonal to the normal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Face {
    const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    fn normal(self) -> i8 {
        match self {
            Face::PosX => 0,
            Face::NegX => 1,
            Face::PosY => 2,
            Face::NegY => 3,
            Face::PosZ => 4,
            Face::NegZ => 5,
        }
    }

    // Which axis is perpendicular to this face (the "slice" axis)
    fn axis(self) -> usize {
        match self {
            Face::PosX | Face::NegX => 0,
            Face::PosY | Face::NegY => 1,
            Face::PosZ | Face::NegZ => 2,
        }
    }

    // The two axes that span the face quad
    fn uv_axes(self) -> (usize, usize) {
        match self {
            Face::PosX | Face::NegX => (1, 2), // Y, Z
            Face::PosY | Face::NegY => (0, 2), // X, Z
            Face::PosZ | Face::NegZ => (0, 1), // X, Y
        }
    }

    fn is_positive(self) -> bool {
        matches!(self, Face::PosX | Face::PosY | Face::PosZ)
    }
}

impl Chunk {
    /// Greedy mesher. Produces the minimal set of quads that represents
    /// all visible voxel faces, merging coplanar same-color faces greedily.
    ///
    /// `neighbors` are the 6 directly adjacent chunks in [+X,-X,+Y,-Y,+Z,-Z]
    /// order. Pass `None` for a neighbor that hasn't loaded yet — faces on
    /// that boundary will be treated as visible (conservative).
    pub fn mesh(&self, neighbors: [Option<&Chunk>; 6]) -> MeshData {
        if self.is_empty() {
            return MeshData::default();
        }

        let mut mesh = MeshData::default();

        for face in Face::ALL {
            self.mesh_face(face, &neighbors, &mut mesh);
        }

        mesh
    }

    fn mesh_face(&self, face: Face, neighbors: &[Option<&Chunk>; 6], mesh: &mut MeshData) {
        let axis = face.axis();
        let (u_ax, v_ax) = face.uv_axes();
        let s = CHUNK_SIZE;

        let mut visited = vec![false; s * s];

        for d in 0..s {
            // Reset visited for each new slice
            visited.iter_mut().for_each(|x| *x = false);

            for u in 0..s {
                for v in 0..s {
                    // Already consumed by a previous quad on this slice — skip
                    if visited[u * s + v] {
                        continue;
                    }

                    let mut coord = [0usize; 3];
                    coord[axis] = d;
                    coord[u_ax] = u;
                    coord[v_ax] = v;
                    let [x, y, z] = coord;
                    let idx = x + y * s + z * s * s;

                    // Not a visible face — mark visited so we don't check again
                    // and move on. Do NOT emit a quad.
                    if !self.is_active(idx) || !self.face_visible(x, y, z, face, neighbors) {
                        visited[u * s + v] = true;
                        continue;
                    }

                    let voxel = self.voxels[idx];

                    // Expand along v axis first
                    let mut quad_v = 1;
                    while v + quad_v < s {
                        let mut nc = [0usize; 3];
                        nc[axis] = d;
                        nc[u_ax] = u;
                        nc[v_ax] = v + quad_v;
                        let [nx, ny, nz] = nc;
                        let nidx = nx + ny * s + nz * s * s;

                        if visited[u * s + (v + quad_v)]         // already used
                        || !self.is_active(nidx)             // empty voxel
                        || self.voxels[nidx] != voxel        // different color
                        || !self.face_visible(nx, ny, nz, face, neighbors)
                        // occluded
                        {
                            break;
                        }
                        quad_v += 1;
                    }

                    // Expand along u axis, checking the full v-width each step
                    let mut quad_u = 1;
                    'outer: while u + quad_u < s {
                        for dv in 0..quad_v {
                            let mut nc = [0usize; 3];
                            nc[axis] = d;
                            nc[u_ax] = u + quad_u;
                            nc[v_ax] = v + dv;
                            let [nx, ny, nz] = nc;
                            let nidx = nx + ny * s + nz * s * s;

                            if visited[(u + quad_u) * s + (v + dv)]
                                || !self.is_active(nidx)
                                || self.voxels[nidx] != voxel
                                || !self.face_visible(nx, ny, nz, face, neighbors)
                            {
                                break 'outer;
                            }
                        }
                        quad_u += 1;
                    }

                    // Mark every cell in the merged quad as visited so subsequent
                    // iterations don't treat them as new quad origins
                    for du in 0..quad_u {
                        for dv in 0..quad_v {
                            visited[(u + du) * s + (v + dv)] = true;
                        }
                    }

                    emit_quad(mesh, face, coord, quad_u, quad_v, u_ax, v_ax, voxel);
                }
            }
        }
    }

    /// Returns true if the face of the voxel at (x,y,z) in direction `face`
    /// is exposed to air (i.e. should be rendered).
    fn face_visible(
        &self,
        x: usize,
        y: usize,
        z: usize,
        face: Face,
        neighbors: &[Option<&Chunk>; 6],
    ) -> bool {
        let axis = face.axis();
        let positive = face.is_positive();
        let s = CHUNK_SIZE;

        // Coordinate along the face axis
        let d = [x, y, z][axis];

        if positive {
            if d + 1 < s {
                // Neighbor is within this chunk
                let mut nc = [x, y, z];
                nc[axis] += 1;
                let [nx, ny, nz] = nc;
                let nidx = nx + ny * s + nz * s * s;
                !self.is_active(nidx)
            } else {
                // Neighbor is in the adjacent chunk
                // Face index: PosX=0, NegX=1, PosY=2, NegY=3, PosZ=4, NegZ=5
                let neighbor_idx = face as usize;
                match neighbors[neighbor_idx] {
                    None => true, // treat unloaded chunk as empty — show face
                    Some(neighbor) => {
                        // The neighbor voxel is at coordinate 0 on its axis
                        let mut nc = [x, y, z];
                        nc[axis] = 0;
                        let [nx, ny, nz] = nc;
                        let nidx = nx + ny * s + nz * s * s;
                        !neighbor.is_active(nidx)
                    }
                }
            }
        } else {
            if d > 0 {
                let mut nc = [x, y, z];
                nc[axis] -= 1;
                let [nx, ny, nz] = nc;
                let nidx = nx + ny * s + nz * s * s;
                !self.is_active(nidx)
            } else {
                let neighbor_idx = face as usize;
                match neighbors[neighbor_idx] {
                    None => true,
                    Some(neighbor) => {
                        // The neighbor voxel is at coordinate CHUNK_SIZE-1 on its axis
                        let mut nc = [x, y, z];
                        nc[axis] = s - 1;
                        let [nx, ny, nz] = nc;
                        let nidx = nx + ny * s + nz * s * s;
                        !neighbor.is_active(nidx)
                    }
                }
            }
        }
    }
}

/// Emits two triangles (6 indices, 4 vertices) for a merged quad.
/// `origin` is the 3D chunk-local coordinate of the quad's corner voxel.
/// `quad_u` and `quad_v` are the dimensions in voxels along u/v axes.
fn emit_quad(
    mesh: &mut MeshData,
    face: Face,
    origin: [usize; 3],
    quad_u: usize,
    quad_v: usize,
    u_ax: usize,
    v_ax: usize,
    voxel: Voxel,
) {
    let normal = face.normal();
    let color = [voxel.r(), voxel.g(), voxel.b()];

    // The quad sits on the face of the voxel, offset by 1 in the normal
    // direction for positive faces so it sits flush with the voxel surface.
    let face_offset = if face.is_positive() { 1.0 } else { 0.0 };

    // Build the 4 corners of the quad in world-local float coordinates.
    // o = origin corner, then step along u and v by quad_u / quad_v voxels.
    let corner = |du: usize, dv: usize| -> [f32; 3] {
        let mut pos = [0.0f32; 3];
        pos[face.axis()] = origin[face.axis()] as f32 + face_offset;
        pos[u_ax] = (origin[u_ax] + du) as f32;
        pos[v_ax] = (origin[v_ax] + dv) as f32;
        pos
    };

    // Four corners: bottom-left, bottom-right, top-right, top-left
    let v0 = corner(0, 0);
    let v1 = corner(quad_u, 0);
    let v2 = corner(quad_u, quad_v);
    let v3 = corner(0, quad_v);

    let base = mesh.vertices.len() as u32;

    mesh.vertices.extend_from_slice(&[
        VoxelVertex::new(v0, normal, color),
        VoxelVertex::new(v1, normal, color),
        VoxelVertex::new(v2, normal, color),
        VoxelVertex::new(v3, normal, color),
    ]);

    // Winding order: counter-clockwise when viewed from outside
    // Flip winding for negative faces so normals point outward correctly
    // Correct for flipped Y axis
    if matches!(face, Face::PosX | Face::NegY | Face::PosZ) {
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices
            // .extend_from_slice(&[base, base + 3, base + 2, base, base + 2, base + 1]);
            .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}
