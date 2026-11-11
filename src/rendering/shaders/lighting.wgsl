#define_import_path fmc::lighting

#import bevy_pbr::mesh_view_bindings::lights
#import bevy_pbr::mesh_view_types::Fog

const VOID_COLOR: vec3<f32> = vec3<f32>(0.0003, 0.0005, 0.0012);

fn get_light(
    light_texture: texture_3d<f32>,
    _light_texture_sampler: sampler,
    origin: vec3<i32>,
    world_position: vec3<f32>,
    normal: vec3<f32>,
) -> f32 {
    let block_position = vec3<i32>(floor(world_position + normal * 0.01)) + origin;
    let block_light = load_light(light_texture, block_position);

    let sun = pow(0.7, 15.0 - block_light.r * 15.0) * brightness();
    let artificial = pow(0.7, 15.0 - block_light.g * 15.0);
    return max(artificial, sun);
}

//fn sunlight() -> f32 {
//    // Clamps the time of day to -0.3..0.2 and maps it to 0..1. 
//    // The range is chosen so that the light will linger until the sun's halo is
//    // gone but reach full brightness quickly at sunrise.
//    let dawn_progress = saturate((sun_height() + 0.3) / 0.5);
//    // Exponential so it drops quickly at sunset to warn the player that night
//    // is coming. The floor keeps sunlit areas a bit brighter at night.
//    return max(pow(dawn_progress, 4.0), 0.05);
//}

fn fog_color(fog: Fog) -> vec4<f32> {
    return vec4(mix(VOID_COLOR, fog.base_color.rgb, brightness()), fog.base_color.a);
}

fn brightness() -> f32 {
    return smoothstep(-0.15, 0.50, sun_height());
}

fn sun_height() -> f32 {
    return lights.ambient_color.a;
}

// WGSL's `%` is the truncated remainder, so it keeps the sign of the dividend. This folds the
// result back into 0..modulus, matching Rust's rem_euclid, so negative positions wrap.
fn rem_euclid(value: vec3<i32>, modulus: vec3<i32>) -> vec3<i32> {
    return (value % modulus + modulus) % modulus;
}

fn load_light(light_texture: texture_3d<f32>, block_position: vec3<i32>) -> vec2<f32> {
    let texture_size = vec3<i32>(textureDimensions(light_texture));
    return textureLoad(light_texture, rem_euclid(block_position, texture_size), 0).rg;
}
