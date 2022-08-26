use std::collections::BTreeMap;

use bevy::{
    prelude::*,
    render::{
        mesh::VertexAttributeValues,
        render_resource::{PrimitiveTopology, ShaderType, StorageBuffer},
        renderer::{RenderDevice, RenderQueue},
        Extract, RenderApp, RenderStage,
    },
    utils::{HashMap, HashSet},
};
use bvh::{aabb::Bounded, bounding_hierarchy::BHShape, bvh::BVH};
use itertools::Itertools;

pub struct BindlessMeshPlugin;
impl Plugin for BindlessMeshPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<BindlessMeshes>()
                .init_resource::<BindlessMeshMeta>()
                .init_resource::<ExtractedBindlessMeshes>()
                .init_resource::<RenderBindlessMeshes>()
                .add_system_to_stage(RenderStage::Extract, extract_bindless_meshes)
                .add_system_to_stage(RenderStage::Prepare, prepare_bindless_meshes);
        }
    }
}

#[derive(Default, Clone, Copy, ShaderType)]
pub struct GpuVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

#[derive(Debug, Default, Clone, Copy, ShaderType)]
pub struct GpuPrimitive {
    /// Global positions of vertices.
    pub vertices: [Vec3; 3],
    /// Indices of vertices in the vertex buffer (offset not applied).
    pub indices: [u32; 3],
    /// Index of the node in the node buffer (offset not applied).
    node_index: u32,
}

impl Bounded for GpuPrimitive {
    fn aabb(&self) -> bvh::aabb::AABB {
        bvh::aabb::AABB::empty()
            .grow(&self.vertices[0].to_array().into())
            .grow(&self.vertices[1].to_array().into())
            .grow(&self.vertices[2].to_array().into())
    }
}

impl BHShape for GpuPrimitive {
    fn set_bh_node_index(&mut self, index: usize) {
        self.node_index = index as u32;
    }

    fn bh_node_index(&self) -> usize {
        self.node_index as usize
    }
}

#[derive(Default, Clone, ShaderType)]
pub struct GpuNode {
    pub min: Vec3,
    pub max: Vec3,
    pub entry_index: u32,
    pub exit_index: u32,
    pub face_index: u32,
}

#[derive(Default, ShaderType)]
pub struct GpuVertexBuffer {
    #[size(runtime)]
    pub data: Vec<GpuVertex>,
}

#[derive(Default, ShaderType)]
pub struct GpuPrimitiveBuffer {
    #[size(runtime)]
    pub data: Vec<GpuPrimitive>,
}

#[derive(Default, ShaderType)]
pub struct GpuNodeBuffer {
    #[size(runtime)]
    pub data: Vec<GpuNode>,
}

#[derive(Default)]
pub struct BindlessMeshMeta {
    pub vertex_buffer: StorageBuffer<GpuVertexBuffer>,
    pub primitive_buffer: StorageBuffer<GpuPrimitiveBuffer>,
    pub node_buffer: StorageBuffer<GpuNodeBuffer>,
}

#[derive(Debug, Clone)]
pub struct GpuBindlessMesh {
    pub vertex_offset: u32,
    pub primitive_offset: u32,
    pub node_offset: u32,
}

// #[derive(Debug, TypeUuid, Clone, Deref, DerefMut)]
// #[uuid = "d5cd37e2-e015-4415-bc67-cfb7ceba0b26"]
// pub struct BindlessMesh(Mesh);

// impl<T: Into<Mesh>> From<T> for BindlessMesh {
//     fn from(t: T) -> Self {
//         Self(t.into())
//     }
// }

pub struct BindlessMesh {
    pub vertices: Vec<GpuVertex>,
    pub primitives: Vec<GpuPrimitive>,
    pub nodes: Vec<GpuNode>,
}

#[derive(Debug)]
pub enum BindlessMeshError {
    MissAttributePosition,
    MissAttributeNormal,
    MissAttributeUV,
    IncompatiblePrimitiveTopology,
}

fn extract_mesh(mesh: &Mesh) -> Result<BindlessMesh, BindlessMeshError> {
    let positions = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(VertexAttributeValues::as_float3)
        .ok_or(BindlessMeshError::MissAttributePosition)?;
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(VertexAttributeValues::as_float3)
        .ok_or(BindlessMeshError::MissAttributeNormal)?;
    let uvs = mesh
        .attribute(Mesh::ATTRIBUTE_UV_0)
        .and_then(|attribute| match attribute {
            VertexAttributeValues::Float32x2(value) => Some(value),
            _ => None,
        })
        .ok_or(BindlessMeshError::MissAttributeUV)?;

    let mut vertices = vec![];
    for (position, normal, uv) in itertools::multizip((positions, normals, uvs)) {
        vertices.push(GpuVertex {
            position: Vec3::from_slice(position),
            normal: Vec3::from_slice(normal),
            uv: Vec2::from_slice(uv),
        });
    }

    let indices = match mesh.indices() {
        Some(indices) => indices.iter().collect_vec(),
        None => vertices.iter().enumerate().map(|(id, _)| id).collect_vec(),
    };

    let mut faces = match mesh.primitive_topology() {
        PrimitiveTopology::TriangleList => {
            let mut faces = vec![];
            for chunk in &indices.iter().chunks(3) {
                let (v0, v1, v2) = chunk
                    .cloned()
                    .next_tuple()
                    .ok_or(BindlessMeshError::IncompatiblePrimitiveTopology)?;
                let vertices = [v0, v1, v2]
                    .map(|id| vertices[id])
                    .map(|vertex| vertex.position);
                let indices = [v0, v1, v2].map(|id| id as u32);
                faces.push(GpuPrimitive {
                    vertices,
                    indices,
                    node_index: 0,
                });
            }
            Ok(faces)
        }
        PrimitiveTopology::TriangleStrip => {
            let mut faces = vec![];
            for (id, (v0, v1, v2)) in indices.iter().cloned().tuple_windows().enumerate() {
                let indices = if id & 1 == 0 {
                    [v0, v1, v2]
                } else {
                    [v1, v0, v2]
                };
                let vertices = indices.map(|id| vertices[id]).map(|vertex| vertex.position);
                let indices = indices.map(|id| id as u32);
                faces.push(GpuPrimitive {
                    vertices,
                    indices,
                    node_index: 0,
                })
            }
            Ok(faces)
        }
        _ => Err(BindlessMeshError::IncompatiblePrimitiveTopology),
    }?;

    let bvh = BVH::build(&mut faces);
    let nodes = bvh.flatten_custom(&|aabb, entry_index, exit_index, face_index| GpuNode {
        min: aabb.min.to_array().into(),
        max: aabb.max.to_array().into(),
        entry_index,
        exit_index,
        face_index,
    });

    Ok(BindlessMesh {
        vertices,
        primitives: faces,
        nodes,
    })
}

#[derive(Default)]
pub struct ExtractedBindlessMeshes {
    extracted: Vec<(Handle<Mesh>, BindlessMesh)>,
    removed: Vec<Handle<Mesh>>,
}

#[derive(Default, Deref, DerefMut)]
pub struct BindlessMeshes(BTreeMap<Handle<Mesh>, BindlessMesh>);

#[derive(Default, Deref, DerefMut)]
pub struct RenderBindlessMeshes(HashMap<Handle<Mesh>, GpuBindlessMesh>);

fn extract_bindless_meshes(
    mut commands: Commands,
    mut events: Extract<EventReader<AssetEvent<Mesh>>>,
    assets: Extract<Res<Assets<Mesh>>>,
) {
    let mut changed_assets = HashSet::default();
    let mut removed = Vec::new();
    for event in events.iter() {
        match event {
            AssetEvent::Created { handle } | AssetEvent::Modified { handle } => {
                changed_assets.insert(handle.clone_weak());
            }
            AssetEvent::Removed { handle } => {
                changed_assets.remove(handle);
                removed.push(handle.clone_weak());
            }
        }
    }

    let mut extracted = Vec::new();
    for handle in changed_assets.drain() {
        if let Some(mesh) = assets.get(&handle).and_then(|mesh| extract_mesh(mesh).ok()) {
            extracted.push((handle, mesh));
        }
    }

    commands.insert_resource(ExtractedBindlessMeshes { extracted, removed });
}

fn prepare_bindless_meshes(
    mut extracted_assets: ResMut<ExtractedBindlessMeshes>,
    mut meta: ResMut<BindlessMeshMeta>,
    mut meshes: ResMut<BindlessMeshes>,
    mut render_meshes: ResMut<RenderBindlessMeshes>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let mut dirty = false;

    for (handle, mesh) in extracted_assets.extracted.drain(..) {
        dirty = true;
        meshes.insert(handle, mesh);
    }

    for handle in extracted_assets.removed.drain(..) {
        dirty = true;
        meshes.remove(&handle);
    }

    if dirty {
        meta.vertex_buffer.get_mut().data.clear();
        meta.primitive_buffer.get_mut().data.clear();
        meta.node_buffer.get_mut().data.clear();

        for (handle, mesh) in meshes.iter() {
            let vertex_offset = meta.vertex_buffer.get().data.len() as u32;
            meta.vertex_buffer
                .get_mut()
                .data
                .append(&mut mesh.vertices.clone());

            let primitive_offset = meta.primitive_buffer.get().data.len() as u32;
            meta.primitive_buffer
                .get_mut()
                .data
                .append(&mut mesh.primitives.clone());

            let node_offset = meta.node_buffer.get().data.len() as u32;
            meta.node_buffer
                .get_mut()
                .data
                .append(&mut mesh.nodes.clone());

            render_meshes.insert(
                handle.clone_weak(),
                GpuBindlessMesh {
                    vertex_offset,
                    primitive_offset,
                    node_offset,
                },
            );
        }

        meta.vertex_buffer
            .write_buffer(&render_device, &render_queue);
        meta.primitive_buffer
            .write_buffer(&render_device, &render_queue);
        meta.node_buffer.write_buffer(&render_device, &render_queue);
    }
}
