#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#import bevy_pbr::mesh_view_bindings::{view, lights};
#import bevy_pbr::mesh_view_types::{OrderIndependentTransparencySettings, Fog}

#ifdef OIT_ENABLED
// TODO: This is a copy of bevy_core_pipeline::oit::oit_draw because it doesn't like
// that the oit bindings are redefined.
fn oit_draw(position: vec4f, color: vec4f) {
    // Don't add fully transparent fragments to the list
    // because we don't want to have to sort them in the resolve pass
    if color.a < oit_settings.alpha_threshold {
        return;
    }
    //get the index of the current fragment relative to the screen size
    let screen_index = i32(floor(position.x) + floor(position.y) * view.viewport.z);
    // get the size of the buffer.
    // It's always the size of the screen
    let buffer_size = i32(view.viewport.z * view.viewport.w);

    // gets the layer index of the current fragment
    var layer_id = atomicAdd(&oit_layer_ids[screen_index], 1);
    // exit early if we've reached the maximum amount of fragments per layer
    if layer_id >= oit_settings.layers_count {
        // force to store the oit_layers_count to make sure we don't
        // accidentally increase the index above the maximum value
        atomicStore(&oit_layer_ids[screen_index], oit_settings.layers_count);
        // TODO for tail blending we should return the color here
        return;
    }

    // get the layer_index from the screen
    let layer_index = screen_index + layer_id * buffer_size;
    let rgb9e5_color = bevy_pbr::rgb9e5::vec3_to_rgb9e5_(color.rgb);
    let depth_alpha = pack_24bit_depth_8bit_alpha(position.z, color.a);
    oit_layers[layer_index] = vec2(rgb9e5_color, depth_alpha);
}
#endif // OIT_ENABLED

fn pack_24bit_depth_8bit_alpha(depth: f32, alpha: f32) -> u32 {
    let depth_bits = u32(saturate(depth) * f32(0xFFFFFFu) + 0.5);
    let alpha_bits = u32(saturate(alpha) * f32(0xFFu) + 0.5);
    return (depth_bits & 0xFFFFFFu) | ((alpha_bits & 0xFFu) << 24u);
}

fn unpack_24bit_depth_8bit_alpha(packed: u32) -> vec2<f32> {
    let depth_bits = packed & 0xFFFFFFu;
    let alpha_bits = (packed >> 24u) & 0xFFu;
    return vec2(f32(depth_bits) / f32(0xFFFFFFu), f32(alpha_bits) / f32(0xFFu));
}

@group(0) @binding(2) var<uniform> fog: Fog;
@group(0) @binding(3) var cloud_coverage_texture: texture_2d<f32>;
@group(0) @binding(4) var cloud_depth_texture: texture_2d<f32>;
@group(0) @binding(5) var screen_texture: texture_2d<f32>;
@group(0) @binding(6) var texture_sampler: sampler;
#ifdef OIT_ENABLED
@group(0) @binding(7) var<storage, read_write> oit_layers: array<vec2<u32>>;
@group(0) @binding(8) var<storage, read_write> oit_layer_ids: array<atomic<i32>>;
@group(0) @binding(9) var<uniform> oit_settings: OrderIndependentTransparencySettings;
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
