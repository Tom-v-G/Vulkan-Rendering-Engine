use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use cgmath::{vec2, vec3};

use anyhow::Result;
use tobj::load_obj_buf;

use crate::app_data::AppData;
use crate::vertex::Vertex;

pub fn load_model(data: &mut AppData) -> Result<()> {
    // let mut reader = BufReader::new(File::open("resources/frog.obj")?);
    let mut reader = BufReader::new(File::open("resources/viking_room.obj")?);

    let (models, _) = load_obj_buf(
        &mut reader,
        &tobj::LoadOptions {
            triangulate: true,
            ..Default::default()
        },
        |_| Ok(Default::default()),
    )?;

    // hashmap to prevent duplicate vertices
    let mut unique_vertices = HashMap::new();

    for model in &models {
        // normalization variables
        let mut x_min: f32 = 0.0;
        let mut x_max: f32 = 0.0;
        let mut y_min: f32 = 0.0;
        let mut y_max: f32 = 0.0;
        let mut z_min: f32 = 0.0;
        let mut z_max: f32 = 0.0;

        // find bounds of model
        for index in &model.mesh.indices {
            let pos_offset = (3 * index) as usize;
            // x
            if model.mesh.positions[pos_offset] < x_min {
                x_min = model.mesh.positions[pos_offset]
            }
            if model.mesh.positions[pos_offset] > x_max {
                x_max = model.mesh.positions[pos_offset]
            }
            // y
            if model.mesh.positions[pos_offset + 1] < y_min {
                y_min = model.mesh.positions[pos_offset + 1]
            }
            if model.mesh.positions[pos_offset] > y_max {
                y_max = model.mesh.positions[pos_offset + 1]
            }
            // z
            if model.mesh.positions[pos_offset + 2] < z_min {
                z_min = model.mesh.positions[pos_offset + 2]
            }
            if model.mesh.positions[pos_offset + 2] > z_max {
                z_max = model.mesh.positions[pos_offset + 2]
            }
        }

        // (x_mesh_bounds, y_mesh_bounds, z_mesh_bounds) = find_model_vertex_bounds(&model.mesh);

        for index in &model.mesh.indices {
            let pos_offset = (3 * index) as usize;
            let tex_coord_offset = (2 * index) as usize;

            // TODO: make normalization correct
            let vertex = Vertex {
                pos: vec3(
                    (((model.mesh.positions[pos_offset] - x_min) / (x_max - x_min).abs()) - 0.5)
                        * 2.0,
                    (((model.mesh.positions[pos_offset + 1] - y_min) / (y_max - y_min).abs())
                        - 0.5)
                        * 2.0,
                    (((model.mesh.positions[pos_offset + 2] - z_min) / (z_max - z_min).abs())
                        - 0.5)
                        * 2.0,
                ),
                color: vec3(1.0, 1.0, 1.0),
                tex_coord: vec2(
                    model.mesh.texcoords[tex_coord_offset],
                    1.0 - model.mesh.texcoords[tex_coord_offset + 1],
                ),
            };

            if let Some(index) = unique_vertices.get(&vertex) {
                data.indices.push(*index as u32);
            } else {
                let index = data.vertices.len();
                unique_vertices.insert(vertex, index);
                data.vertices.push(vertex);
                data.indices.push(index as u32);
            }
        }
    }
    Ok(())
}
