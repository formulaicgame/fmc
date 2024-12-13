#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    utils::{
        rand_range_u,
        rand_f
    }
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
#ifdef VERTEX_COLORS
    @location(2) lighting: vec4<f32>
#endif
}

struct ParticleMaterialUniform {
    block_texture: u32,
    base_color: vec4<f32>,
}

@group(2) @binding(0)
var<uniform> material: ParticleMaterialUniform;
@group(2) @binding(1)
var texture: texture_2d<f32>;
@group(2) @binding(2)
var texture_sampler: sampler;

const PIXEL_SIZE: f32 = 16.0;

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) uv: vec2<f32>,
#ifdef VERTEX_COLORS
    @location(5) lighting: vec4<f32>
#endif
) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);

#ifdef VERTEX_COLORS
    out.lighting = lighting;
#endif

    // TODO: Something like this is necessary for instancing, but trying to
    // uniquely identify an instance like this doesn't work. The instance
    // indices are not stable and will shift with each frame. 
    //if material.block_texture != 0 {
    //    var state = instance_index;
    //    // The particle will show 2 to 4 pixels of the texture
    //    let size = 2u + rand_range_u(3u, &state);
    //    let offset = f32(rand_range_u(16u - size, &state)) / PIXEL_SIZE;
    //    out.uv = uv * f32(size) / PIXEL_SIZE + offset;
    //}
    out.uv = uv;

    return out;
}

@fragment
fn fragment(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    var output_color: vec4<f32>;

    if material.block_texture != 0 {
        output_color = textureSample(texture, texture_sampler, in.uv);
    }

    output_color = output_color * material.base_color;

#ifdef VERTEX_COLORS
    output_color = output_color * in.lighting;
#endif

    return output_color;
}
