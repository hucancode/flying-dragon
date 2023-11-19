use crate::geometry::Vertex;
use std::io::BufReader;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, Device};

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>, device: &Device) -> Self {
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });
        Self {
            vertices,
            indices,
            vertex_buffer,
            index_buffer,
        }
    }
    pub fn load_obj(source: &[u8], device: &Device) -> Self {
        let mut reader = BufReader::new(source);
        if let Ok((models, _materials)) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                single_index: true,
                ..Default::default()
            },
            |_matpath| Err(tobj::LoadError::GenericFailure),
        ) {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            for model in models {
                let mesh = model.mesh;
                // println!("pos: {:?}", mesh.positions.len());
                // println!("index: {:?}", mesh.normal_indices.len());
                // println!("index: {:?}", mesh.indices.len());
                let offset = vertices.len();
                let n = mesh.positions.len();
                for i in 0..n / 3 {
                    let i = i * 3;
                    let pos = [
                        mesh.positions[i],
                        mesh.positions[i + 1],
                        mesh.positions[i + 2],
                    ];
                    let nor = if mesh.normals.len() <= i + 2 {
                        [0.0, 0.0, 1.0]
                    } else {
                        [mesh.normals[i], mesh.normals[i + 1], mesh.normals[i + 2]]
                    };
                    let col = 0xffff00ff;
                    vertices.push(Vertex::new(pos, nor, col));
                }
                for i in mesh.indices {
                    indices.push(offset as u32 + i as u32);
                }
            }
            Self::new(vertices, indices, device)
        } else {
            Self::new(Vec::new(), Vec::new(), device)
        }
    }
}
