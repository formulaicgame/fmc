#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    pbr_functions::premultiply_alpha,
    pbr_types,
    mesh_view_bindings::{globals, lights, view, fog, oit_layers, oit_layer_ids, oit_settings},
    mesh_view_types::{FOG_MODE_OFF, Fog}
}

#import bevy_core_pipeline::tonemapping::{
    screen_space_dither,
    tone_mapping
} 

#import bevy_render::maths::{powsafe, HALF_PI}

#ifdef OIT_ENABLED
    #import bevy_core_pipeline::oit::oit_draw
#endif // OIT_ENABLED

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) packed_bits: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) texture_index: i32,
    @location(4) light: u32,
};

// NOTE: 0,0 is top left corner
const UVS: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    out.light = (vertex.packed_bits >> 22u) & 0xFFu;
    out.texture_index = i32(vertex.packed_bits & 0x0007FFFFu);

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);

    // Derive which face of the block is being rendered from the normal. This can again be used to
    // derive the uv of the vertex by combining it with which corner it is and the world position.
    // If the block is made from custom vertices care must be taken to lign up its texture
    // with its position in the block.
    //let abs_normal: vec3<f32> = abs(out.world_normal);
    //let max_normal = max(max(abs_normal.x, abs_normal.y), abs_normal.z);
    //if max_normal == abs_normal.x {
    //    out.uv = out.world_position.zy;
    //} else if max_normal == abs_normal.y {
    //    out.uv = out.world_position.xz;
    //} else  {
    //    out.uv = out.world_position.xy;
    //}

    //// TODO: Naga might allow indexing without const value in the future
    //let uv_index: u32 = (vertex.packed_bits & 0x180000u) >> 19u;
    //if uv_index == 0u {
    //    // Top left corner
    //    out.uv = vec2(
    //        // fract is x - x.floor() so it's inversed for negative numbers
    //        fract(out.uv.x),
    //        fract(out.uv.y)
    //    );
    //} else if uv_index == 1u {
    //    // Bottom left corner
    //    out.uv = vec2(
    //        fract(out.uv.x),
    //        // Since this is on the high side of the range extracting the fract is harder since it
    //        // can be a whole number. e.g a position of 1.0 should give a fraction of 1.0, not 0.0.
    //        out.uv.y - (ceil(out.uv.y) - 1)
    //    );
    //} else if uv_index == 2u {
    //    // Top right corner
    //    out.uv = vec2(
    //        out.uv.x - (ceil(out.uv.x) - 1),
    //        fract(out.uv.y)
    //    );
    //} else if uv_index == 3u {
    //    // Bottom right corner
    //    out.uv = vec2(
    //        out.uv.x - (ceil(out.uv.x) - 1),
    //        out.uv.y - (ceil(out.uv.y) - 1)
    //    );
    //}

    out.uv = vertex.uv;

    let rotate_uv = bool((vertex.packed_bits & 0x200000u) >> 21u);
    if rotate_uv {
        out.uv = vec2<f32>(
            0.5 + cos(0.25 * HALF_PI) * (out.uv.x - 0.5) + sin(0.25 * HALF_PI) * (out.uv.y - 0.5),
            0.5 - sin(0.25 * HALF_PI) * (out.uv.x - 0.5) + cos(0.25 * HALF_PI) * (out.uv.y - 0.5),
        );
    }

    //let rotation = f32((vertex.packed_bits & 0x38000000u) >> 27u);
    //let rotation = 7.0;

    //let out_position = vec4<f32>(
    //    center.x + cos(rotation * HALF_PI) * (vertex.position.x - center.x) - sin(rotation * HALF_PI) * (vertex.position.z - center.z),
    //    vertex.position.y,
    //    center.z + sin(rotation * HALF_PI) * (vertex.position.x - center.x) + sin(rotation * HALF_PI) * (vertex.position.z - center.z),
    //    1.0
    //);

    return out;
}

struct BlockMaterial {
    base_color: vec4<f32>,
    flags: u32,
    alpha_cutoff: f32,
    animation_frames: u32,
};

@group(2) @binding(0)
var<uniform> material: BlockMaterial;
@group(2) @binding(1)
var texture_array: texture_2d_array<f32>;
@group(2) @binding(2)
var texture_array_sampler: sampler;

@fragment
fn fragment(
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) texture_index: i32,
    @location(4) light_packed: u32,
) -> @location(0) vec4<f32> {
    var output_color: vec4<f32> = material.base_color;

    // TODO: For some reason this refuses to take a u32 as the index
    let fps = 10.0;
    let texture_index_animation_offset: i32 = texture_index + i32(globals.time * fps) % i32(material.animation_frames);
    output_color = output_color * textureSample(texture_array, texture_array_sampler, uv, texture_index_animation_offset);

    let artificial_level = f32(light_packed & 0xFu);
    let sunlight_level = f32((light_packed >> 4u) & 0xFu);

    let artificial = (pow(0.8, 15.0 - artificial_level));
    let sunlight = pow(0.8, 15.0 - sunlight_level) * lights.ambient_color.a;
    let light = max(artificial, sunlight);

    output_color = vec4(output_color.rgb * light, output_color.a);

    if abs(world_normal.z) >= 0.99 {
        output_color = vec4(output_color.rgb * 0.8, output_color.a);
    } else if abs(world_normal.x) >= 0.99 {
        output_color = vec4(output_color.rgb * 0.5, output_color.a);
    } else if world_normal.y <= -0.99 {
        output_color = vec4(output_color.rgb * 0.3, output_color.a);
    }

    output_color = alpha_discard(material, output_color);

    // This is water depth, hard to figure out, don't know if useless, no delete.
    //if ((material.flags & STANDARD_MATERIAL_FLAGS_IS_WATER) != 0u) {
    //    let z_depth_ndc = prepass_depth(frag_coord, sample_index);
    //    let z_depth_buffer_view = view.projection[3][2] / z_depth_ndc;
    //    let z_fragment_view = view.projection[3][2] / frag_coord.z;
    //    let diff = z_fragment_view - z_depth_buffer_view;
    //    let alpha = min(exp(-diff * 0.08 - 1.0), 1.0);
    //    output_color.a = alpha;
    //}

#ifdef DISTANCE_FOG
    //if ((material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT) != 0u) {
    var fog_copy = fog;
    // TODO: Tinting the rgb of the ambient light at sunrise/sunset could
    // be nice? Or access to the sun position is needed, but we can't have that.
    let brightness = smoothstep(0.0, 0.10, lights.ambient_color.a);
    fog_copy.base_color = vec4(fog_copy.base_color.rgb * brightness, fog_copy.base_color.a);
    output_color = apply_fog(fog_copy, output_color, world_position);
    //}
#endif // DISTANCE_FOG

#ifdef OIT_ENABLED
    // OIT_ENABLED is only set when the alpha mode is AlphaMode::Blend, but
    // check anyway in case they change it.
    let alpha_mode = material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        oit_draw(frag_coord, output_color);
        discard;
    }
#endif // OIT_ENABLED

#ifdef TONEMAP_IN_SHADER
    // TODO: Makes it look so bland...
    //output_color = tone_mapping(output_color, view.color_grading);
#ifdef DEBAND_DITHER
    var output_rgb = output_color.rgb;
    output_rgb = powsafe(output_rgb, 1.0 / 2.2);
    output_rgb += screen_space_dither(frag_coord.xy);
    // This conversion back to linear space is required because our output texture format is
    // SRGB; the GPU will assume our output is linear and will apply an SRGB conversion.
    output_rgb = powsafe(output_rgb, 2.2);
    output_color = vec4(output_rgb, output_color.a);
#endif
#endif
#ifdef PREMULTIPLY_ALPHA
    output_color = premultiply_alpha(material.flags, output_color);
#endif
    return output_color;
}

fn apply_fog(
    fog_params: Fog,
    input_color: vec4<f32>,
    world_position: vec4<f32>,
) -> vec4<f32> {
    let distance = length(vec3(
        world_position.xz - view.world_position.xz,
        // Makes fog appear below the camera, but not above
        min(world_position.y - view.world_position.y, 0.0))
    );
    var fog_color = fog_params.base_color;
    let start = fog_params.be.x;
    let end = fog_params.be.y;
    fog_color.a *= 1.0 - clamp((end - distance) / (end - start), 0.0, 1.0);
    // TODO: fog_params has a 'mode' field that can be appropriated to apply
    // fogs differently, for now just squared
    fog_color.a *= fog_color.a;
    // The input_color alpha and fog alpha are added to transition water opacity to opaque so that
    // you can only see through it at very sharp angles.
    return vec4<f32>(mix(input_color.rgb, fog_color.rgb, fog_color.a), input_color.a + fog_color.a);
}

fn alpha_discard(material: BlockMaterial, output_color: vec4<f32>) -> vec4<f32> {
    var color = output_color;
    let alpha_mode = material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        // NOTE: If rendering as opaque, alpha should be ignored so set to 1.0
        color.a = 1.0;
    }

#ifdef MAY_DISCARD
    else if alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
        if color.a >= material.alpha_cutoff {
            // NOTE: If rendering as masked alpha and >= the cutoff, render as fully opaque
            color.a = 1.0;
        } else {
            // NOTE: output_color.a < in.material.alpha_cutoff should not be rendered
            discard;
        }
    }
#endif

    return color;
}

// for debug
fn get_light(light: u32) -> f32 {
    // TODO: This would be nice as a constant array, but dynamic indexing is not supported by naga.
    if light == 0u {
        return 0.03;
    } else if light == 1u {
        return 0.04;
    } else if light == 2u {
        return 0.05;
    } else if light == 3u {
        return 0.07;
    } else if light == 4u {
        return 0.09;
    } else if light == 5u {
        return 0.11;
    } else if light == 6u {
        return 0.135;
    } else if light == 7u {
        return 0.17;
    } else if light == 8u {
        return 0.21;
    } else if light == 9u {
        return 0.26;
    } else if light == 10u {
        return 0.38;
    } else if light == 11u {
        return 0.41;
    } else if light == 12u {
        return 0.51;
    } else if light == 13u {
        return 0.64;
    } else if light == 14u {
        return 0.8;
    } else if light == 15u {
        return 1.0;
    } else {
        return 0.0;
    }
}

