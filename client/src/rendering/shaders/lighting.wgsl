#define_import_path fmc::lighting

#import bevy_pbr::mesh_view_bindings::lights

fn get_light(
    light_texture: texture_3d<f32>,
    _light_texture_sampler: sampler,
    origin: vec3<i32>,
    world_position: vec3<f32>,
    normal: vec3<f32>,
) -> f32 {
    let block_position = vec3<i32>(floor(world_position + normal * 0.01)) + origin;
    let block_light = load_light(light_texture, block_position);

    return brightness(block_light, lights.ambient_color.a);
}

fn brightness(light: vec2<f32>, ambient: f32) -> f32 {
    let sunlight = pow(0.8, 15.0 - light.r * 15.0) * ambient;
    let artificial = pow(0.8, 15.0 - light.g * 15.0);
    return max(artificial, sunlight);
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
