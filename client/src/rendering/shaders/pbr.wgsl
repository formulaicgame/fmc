#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_bindings
#import bevy_pbr::pbr_types
#import bevy_pbr::prepass_utils
#import bevy_pbr::view_transformations

#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_view_bindings::{
    view,
    fog,
    screen_space_ambient_occlusion_texture
}
#import bevy_pbr::mesh_view_types::FOG_MODE_OFF
#import bevy_render::maths::powsafe
#import bevy_core_pipeline::tonemapping:: {
    screen_space_dither,
    tone_mapping
}
#import bevy_pbr::parallax_mapping::parallaxed_uv
#import bevy_pbr::mesh_view_bindings::lights

#import bevy_pbr::prepass_utils

#ifdef SCREEN_SPACE_AMBIENT_OCCLUSION
#import bevy_pbr::gtao_utils::gtao_multibounce
#endif

struct FragmentInput {
    @builtin(front_facing) is_front: bool,
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(3) world_tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
    @location(5) packed_bits: u32,
};

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    var output_color: vec4<f32> = pbr_bindings::material.base_color;

    let is_orthographic = view.clip_from_view[3].w == 1.0;
    let V = pbr_functions::calculate_view(in.world_position, is_orthographic);

#ifdef VERTEX_UVS
    // TODO: Since this blends with the color below it the perspective is ruined at an angle. Hard to explain
    //       in words, but can be seen easily. Some squares appear larger than expected when indented.
    //       If the texture of the block it is applied to is passed together with the depth map we could offset
    //       the uv position of the color texture to get the accurate color I think.
    //       It's also needed when overlayed on transparent(masked) textures
    if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DEPTH_MAP_BIT) != 0u) {
        // This is taken from https://github.com/DartCat25/resourcepacks/tree/main/3d-breaking
        // I've annotated some of what I understand

        let color = textureSampleBias(pbr_bindings::depth_map_texture, pbr_bindings::depth_map_sampler, in.uv, view.mip_bias);

        // Only non-transparent parts of the texture are used to create the illusion.
        if color.a < 0.1 {
            discard;
        }

        // It uses the view direction(camera -> position) to decide what the offset should be.
        // We transform the view direction so that x and y go along the surface of the face and z is the normal.
        let view_position = view_transformations::position_world_to_view(in.world_position.xyz);
        let view_normal = view_transformations::direction_world_to_view(in.world_normal);
        var x: vec3<f32>;
        var y: vec3<f32>;
        // Why 0.9?
        if abs(in.world_normal.y) >= 0.9 {
            // why negative?
            y = view_transformations::direction_world_to_view(vec3(0.0, 0.0, -1.0));
            x = view_transformations::direction_world_to_view(vec3(-1.0, 0.0, 0.0));
        } else {
            y = view_transformations::direction_world_to_view(vec3(0.0, 1.0, 0.0));
            x = cross(view_normal, y);
        }

        let coordinate_transform = mat3x3(x,y,view_normal);
        let view_direction = normalize(view_position * coordinate_transform);

        let texture_size = textureDimensions(pbr_bindings::depth_map_texture);
        var offset: vec2<f32> = view_direction.xy / view_direction.z / f32(texture_size.x) * 0.5;

        // TODO: I must have indexed the bottom mesh the wrong way around.
        if in.world_normal.y < -0.9 {
            offset.y *= -1.0;
        }

        // TODO: Whenever the offset pushes it off the edge of the texture it doesn't register as alpha == 0 
        //       This causes edges to not be painted correctly.
        //       Maybe this is wanted behaviour though? Just make the texture line up at the edges and it will
        //       kinda look like a little chunk of the block has been removed somewhat?
        //
        // Increment offset until we hit a transparent part of the texture.
        // Color it very dark to create the illusion of being and edge
        for (var i = 1; i <= 16; i++) {
            let uv = in.uv + offset / 16.0 * f32(i);
            let sample = textureSampleBias(pbr_bindings::depth_map_texture, pbr_bindings::depth_map_sampler, uv, view.mip_bias);
            if sample.a < 0.1 {
                output_color = vec4(0.0, 0.0, 0.0, 0.85);
                return output_color;
            }
        }

        // 
        return vec4(0.0, 0.0, 0.0, 0.6);
        //if i > 16 {
        //    let sample = textureSampleBias(pbr_bindings::depth_map_texture, pbr_bindings::depth_map_sampler, uv + offset, view.mip_bias);

        //}
    }
#endif

#ifdef VERTEX_COLORS
    output_color = output_color * in.color;
#endif

    let artificial_level = f32(in.packed_bits & 0xFu);
    var sunlight_level = f32((in.packed_bits >> 4u) & 0xFu);

    // TODO: The 1.2 is a scaling factor to make it look bright enough, idk if it's the models
    // themselves or something else in the shader that makes them darker than they should be.
    let artificial = (pow(0.8, 15.0 - artificial_level)) * 1.2;
    let sunlight = pow(0.8, 15.0 - sunlight_level) * lights.ambient_color.a * 1.2;
    let light = max(artificial, sunlight);

    output_color = vec4(output_color.rgb * light, output_color.a);
    
    // TODO: The bottom is just given a somewhat dark color, but should be significantly darker than the sides.
    //
    // If the face is angled down/up it is given a lighting penalty between -0.2 and 0.2
    // if the face is angled right/left it is given no lighting penalty, but if front/back it is given a penalty of 0.3
    let top_deflection = dot(in.world_normal, vec3(0.0, 1.0, 0.0)) * 0.2;
    // Notice how it inverts the absolute of the dot product. This is so that vertices pointing up and down will
    // result in 1.0 and not 0.0 as their vec2 dot product will always yield zero when x and z = 0.
    let deflection: f32 = 0.5 + (1.0 - abs(dot(vec2(1.0, 0.0), in.world_normal.xz))) * (0.3 + top_deflection);
    output_color = vec4(output_color.rgb * deflection, output_color.a);


#ifdef VERTEX_COLORS
    output_color = output_color * in.color;
#endif

#ifdef VERTEX_UVS
    if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {
        output_color = output_color * textureSampleBias(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, in.uv, view.mip_bias);
    }
#endif

    output_color = pbr_functions::alpha_discard(pbr_bindings::material, output_color);

    // fog
    if (fog.mode != FOG_MODE_OFF && (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT) != 0u) {
        output_color = pbr_functions::apply_fog(fog, output_color, in.world_position.xyz, view.world_position.xyz);
    }

#ifdef TONEMAP_IN_SHADER
    output_color = tone_mapping(output_color, view.color_grading);
#ifdef DEBAND_DITHER
    var output_rgb = output_color.rgb;
    output_rgb = powsafe(output_rgb, 1.0 / 2.2);
    output_rgb = output_rgb + screen_space_dither(in.position.xy);
    // This conversion back to linear space is required because our output texture format is
    // SRGB; the GPU will assume our output is linear and will apply an SRGB conversion.
    output_rgb = powsafe(output_rgb, 2.2);
    output_color = vec4(output_rgb, output_color.a);
#endif
#endif
#ifdef PREMULTIPLY_ALPHA
    output_color = pbr_functions::premultiply_alpha(pbr_bindings::material.flags, output_color);
#endif
    return output_color;
}
