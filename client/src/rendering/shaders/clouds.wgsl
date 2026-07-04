#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#import bevy_pbr::mesh_view_bindings::{view, lights};
#import bevy_pbr::mesh_view_types::{OrderIndependentTransparencySettings, OitFragmentNode, Fog}

#ifdef OIT_ENABLED
// TODO: This is a copy of bevy_core_pipeline::oit::oit_draw because it doesn't like
// that the oit bindings are redefined.
fn oit_draw(position: vec4f, color: vec4f) {
    // Don't add fully transparent fragments to the list
    // because we don't want to have to sort them in the resolve pass
    if color.a < oit_settings.alpha_threshold {
        return;
    }
    // get the index of the current fragment relative to the screen size
    let screen_index = u32(floor(position.x) + floor(position.y) * view.viewport.z);

    let new_node_index = atomicAdd(&oit_atomic_counter, 1u);
    // exit early if we've reached the maximum amount of fragment nodes
    if new_node_index >= oit_nodes_capacity {
        return;
    }

    var node: OitFragmentNode;
    // In `oit_heads`, indices are offset by 1 so that 0 can be the empty sentinel
    // (buffers are zero-initialized). An empty head underflows to the 0xFFFFFFFF
    // list-end sentinel the resolve pass looks for.
    node.next = atomicExchange(&oit_heads[screen_index], new_node_index + 1u) - 1u;
    node.color = bevy_pbr::rgb9e5::vec3_to_rgb9e5_(color.rgb);
    node.depth_alpha = pack_24bit_depth_8bit_alpha(position.z, color.a);
    oit_nodes[new_node_index] = node;
}
#endif // OIT_ENABLED

// Depth is in the high bits so that packed values sort by depth
fn pack_24bit_depth_8bit_alpha(depth: f32, alpha: f32) -> u32 {
    let depth_bits = u32(saturate(depth) * f32(0xFFFFFFu) + 0.5);
    let alpha_bits = u32(saturate(alpha) * f32(0xFFu) + 0.5);
    return (depth_bits << 8u) | alpha_bits;
}

fn unpack_24bit_depth_8bit_alpha(packed: u32) -> vec2<f32> {
    let depth_bits = packed >> 8u;
    let alpha_bits = packed & 0xFFu;
    return vec2(f32(depth_bits) / f32(0xFFFFFFu), f32(alpha_bits) / f32(0xFFu));
}

@group(0) @binding(2) var<uniform> fog: Fog;
@group(0) @binding(3) var cloud_coverage_texture: texture_2d<f32>;
@group(0) @binding(4) var cloud_depth_texture: texture_2d<f32>;
@group(0) @binding(5) var screen_texture: texture_2d<f32>;
@group(0) @binding(6) var texture_sampler: sampler;
#ifdef OIT_ENABLED
@group(0) @binding(7) var<uniform> oit_settings: OrderIndependentTransparencySettings;
@group(0) @binding(8) var<uniform> oit_nodes_capacity: u32;
@group(0) @binding(9) var<storage, read_write> oit_nodes: array<OitFragmentNode>;
@group(0) @binding(10) var<storage, read_write> oit_heads: array<atomic<u32>>;
@group(0) @binding(11) var<storage, read_write> oit_atomic_counter: atomic<u32>;
#endif

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    @builtin(sample_index) sample_index: u32,
    @location(0) uv: vec2<f32>
) -> @location(0) vec4<f32> {
    let screen_color = textureSample(screen_texture, texture_sampler, uv);
    let coverage = textureLoad(cloud_coverage_texture, vec2<i32>(position.xy), i32(sample_index)).r;
    let depth = textureLoad(cloud_depth_texture, vec2<i32>(position.xy), i32(sample_index)).r;

    let opacity_exponent = 1.5;
    let opacity = 0.2;
    let alpha = pow(coverage, opacity_exponent) / (1.0/opacity + pow(coverage, opacity_exponent)-1.0);
    let gradient = smoothstep(0.00, 0.10, lights.ambient_color.a);
    // Night color to day color
    let color = vec3f(mix(0.01, 0.7, gradient));
    let cloud_color = vec4f(color, alpha);

    let p = vec4f(position.x, position.y, depth, 1.0);
#ifdef OIT_ENABLED
    if depth != 0.0 {
        oit_draw(p, cloud_color);
    }
#endif // OIT_ENABLED
    return screen_color;
}
