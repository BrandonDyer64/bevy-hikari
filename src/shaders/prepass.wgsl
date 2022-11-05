#import bevy_hikari::mesh_view_bindings
#import bevy_hikari::utils
#import bevy_pbr::mesh_types

@group(1) @binding(0)
var<uniform> mesh: Mesh;
@group(1) @binding(1)
var<uniform> previous_mesh: PreviousMesh;
@group(1) @binding(2)
var<uniform> instance_index: InstanceIndex;

#import bevy_pbr::mesh_functions

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) previous_world_position: vec4<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var model = mesh.model;
    let vertex_position = vec4<f32>(vertex.position, 1.0);

    var projection = view.projection;
    var jitter = vec2<f32>(0.0);
    let pixel_size = frame.upscale_ratio / vec2<f32>(view.width, view.height);

#ifdef SMAA_TU_4X
    // +-+-+
    // |0| |
    // | |1|
    // +-+-+
    jitter = (frame_jitter(frame.number) - 0.5) * pixel_size;
    jitter += select(1.0, -1.0, frame.number % 2u == 0u) * pixel_size;
#else
#ifdef TEMPORAL_ANTI_ALIASING
    jitter = 2.0 * (frame_jitter(frame.number) - 0.5) * pixel_size;
#endif  // TEMPORAL_ANTI_ALIASING
#endif  // SMAA_TU_4X

    if projection[3].w != 1.0 {
        // Perspective
        projection[2][0] += jitter.x;
        projection[2][1] -= jitter.y;
    }

    var out: VertexOutput;
    out.world_position = mesh_position_local_to_world(model, vertex_position);
    out.previous_world_position = mesh_position_local_to_world(previous_mesh.model, vertex_position);
    out.world_normal = mesh_normal_local_to_world(vertex.normal);
    out.clip_position = projection * view.inverse_view * out.world_position;
    out.uv = vertex.uv;

    if projection[3].w == 1.0 {
        // Orthogonal
        out.clip_position += vec4<f32>(jitter, 0.0, 0.0);
    }

    return out;
}

struct FragmentOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) depth_gradient: vec2<f32>,
    @location(3) instance_material: vec2<u32>,
    @location(4) velocity_uv: vec4<f32>,
};

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let velocity = clip_to_uv(view.view_proj * in.world_position) - clip_to_uv(previous_view.view_proj * in.previous_world_position);

    var out: FragmentOutput;
    out.position = vec4<f32>(in.world_position.xyz, in.clip_position.z);
    out.normal = vec4<f32>(in.world_normal, 1.0);
    out.depth_gradient = vec2<f32>(dpdx(in.clip_position.z), dpdy(in.clip_position.z));
    out.instance_material = vec2<u32>(instance_index.instance, instance_index.material);
    out.velocity_uv = vec4<f32>(velocity, in.uv);
    return out;
}
