#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{lights, globals},
    utils::{
        rand_range_u,
        rand_f
    }
}

struct ParticleMaterialUniform {
    base_color: vec4<f32>,
    lifetime: vec2<f32>,
    random_uv: vec2<u32>,
    spawn_time: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: ParticleMaterialUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var texture: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var texture_sampler: sampler;

struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normals: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) mesh_tag: u32,
}


@vertex
fn vertex(
    in: VertexInput
) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(in.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);

    out.mesh_tag = mesh_functions::get_tag(in.instance_index);

    if material.random_uv.y != 0 {
        var seed = out.mesh_tag >> 8u;
        let range = material.random_uv.y - material.random_uv.x + 1;
        let pixel_size = material.random_uv.x + rand_range_u(range, &seed);
        let texture_dimensions = textureDimensions(texture);
        let offset = f32(rand_range_u(texture_dimensions.x - pixel_size, &seed)) / f32(texture_dimensions.x);
        out.uv = in.uv * f32(pixel_size) / f32(texture_dimensions.x) + offset;
    } else {
        out.uv = in.uv;
    }


    return out;
}

@fragment
fn fragment(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    var output_color: vec4<f32>;

    let artificial_level = f32(in.mesh_tag & 0xFu);
    let sunlight_level = f32((in.mesh_tag & 0xF0) >> 4);

    let artificial = (pow(0.8, 15.0 - artificial_level));
    let sunlight = pow(0.8, 15.0 - sunlight_level) * lights.ambient_color.a;
    let light = max(artificial, sunlight);

    var seed = in.mesh_tag >> 8u;
    let lifetime = material.lifetime.x + (material.lifetime.y - material.lifetime.x) * rand_f(&seed);
    let frames = f32(textureNumLayers(texture));
    let fps = frames / lifetime;
    let frame_index = min(floor((globals.time - material.spawn_time) * fps), frames - 1.0);

    output_color = textureSample(texture, texture_sampler, in.uv, i32(frame_index));
    output_color = output_color * material.base_color;

    // 0.7 to increase contrast
    output_color = vec4(output_color.rgb * light * 0.7, output_color.a);

    if output_color.a < 0.5 {
        discard;
    }

    return output_color;
}
