#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_bindings
#import bevy_pbr::pbr_types
#import bevy_pbr::prepass_utils

#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_view_bindings::{
    view,
    fog,
    screen_space_ambient_occlusion_texture
}
#import bevy_pbr::mesh_view_types::FOG_MODE_OFF
#import bevy_core_pipeline::tonemapping:: {
    screen_space_dither,
    powsafe,
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
#ifdef VERTEX_UVS
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

    let is_orthographic = view.projection[3].w == 1.0;
    let V = pbr_functions::calculate_view(in.world_position, is_orthographic);

#ifdef VERTEX_UVS
    var uv = in.uv;
#ifdef VERTEX_TANGENTS
    if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DEPTH_MAP_BIT) != 0u) {
        let N = in.world_normal;
        let T = in.world_tangent.xyz;
        let B = in.world_tangent.w * cross(N, T);
        // Transform V from fragment to camera in world space to tangent space.
        let Vt = vec3(dot(V, T), dot(V, B), dot(V, N));
        uv = parallaxed_uv(
            pbr_bindings::material.parallax_depth_scale,
            pbr_bindings::material.max_parallax_layer_count,
            pbr_bindings::material.max_relief_mapping_search_steps,
            uv,
            // Flip the direction of Vt to go toward the surface to make the
            // parallax mapping algorithm easier to understand and reason
            // about.
            -Vt,
        );
    }
#endif
#endif

#ifdef VERTEX_COLORS
    output_color = output_color * in.color;
#endif

    let sunlight = (in.packed_bits >> 4u) & 0xFu;
    let artificial_light = in.packed_bits & 0xFu;
    // The object is made both a little brighter at no brightness as well as full brightness to
    // make it contrast better against the terrain.
    let light = pow(0.82, f32(15u - max(sunlight, artificial_light)));

    if sunlight > artificial_light {
        // TODO: This should probably be done for the artifical light too, but I haven't implemented it yet.
        // TODO: The 1.2 is a scaling factor to make it look bright enough, idk if it's the models
        // themselves or something else in the shader that makes them darker than they should be.
        output_color = vec4(output_color.rgb * clamp(light * lights.ambient_color.a, 0.04, 1.0) * 1.2, output_color.a);
    } else {
        output_color = vec4(output_color.rgb * light, output_color.a);
    }
    
    // TODO: The bottom are just given a somewhat dark color, but should be significantly darker than the sides.
    //
    // If the face is angled down/up it is given a lighting penalty between -0.2 and 0.2
    // if the face is angled right/left it is given no lighting penalty, but if front/back it is given a penalty of 0.3
    let top_deflection = dot(in.world_normal, vec3(0.0, 1.0, 0.0)) * 0.2;
    // Notice how it inverts the absolute of the dot product. This is so that vertices pointing up and down will
    // result in a 1.0 and not a 0.0 as their vec2 dot product will always yield zero with their x and z of 0.
    let deflection: f32 = 0.5 + (1.0 - abs(dot(vec2(1.0, 0.0), in.world_normal.xz))) * (0.3 + top_deflection);
    output_color = vec4(output_color.rgb * deflection, output_color.a);


#ifdef VERTEX_COLORS
    output_color = output_color * in.color;
#endif

#ifdef VERTEX_UVS
    if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u) {
        output_color = output_color * textureSampleBias(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, uv, view.mip_bias);
    }
#endif

    // NOTE: Unlit bit not set means == 0 is true, so the true case is if lit
//    if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u) {
//        // Prepare a 'processed' StandardMaterial by sampling all textures to resolve
//        // the material members
//        var pbr_input: pbr_functions::PbrInput;
//
//        pbr_input.material.base_color = output_color;
//        pbr_input.material.reflectance = pbr_bindings::material.reflectance;
//        pbr_input.material.flags = pbr_bindings::material.flags;
//        pbr_input.material.alpha_cutoff = pbr_bindings::material.alpha_cutoff;
//
//        // TODO use .a for exposure compensation in HDR
//        var emissive: vec4<f32> = pbr_bindings::material.emissive;
//#ifdef VERTEX_UVS
//        if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_EMISSIVE_TEXTURE_BIT) != 0u) {
//            emissive = vec4<f32>(emissive.rgb * textureSampleBias(pbr_bindings::emissive_texture, pbr_bindings::emissive_sampler, uv, view.mip_bias).rgb, 1.0);
//        }
//#endif
//        pbr_input.material.emissive = emissive;
//
//        var metallic: f32 = pbr_bindings::material.metallic;
//        var perceptual_roughness: f32 = pbr_bindings::material.perceptual_roughness;
//#ifdef VERTEX_UVS
//        if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_METALLIC_ROUGHNESS_TEXTURE_BIT) != 0u) {
//            let metallic_roughness = textureSampleBias(pbr_bindings::metallic_roughness_texture, pbr_bindings::metallic_roughness_sampler, uv, view.mip_bias);
//            // Sampling from GLTF standard channels for now
//            metallic = metallic * metallic_roughness.b;
//            perceptual_roughness = perceptual_roughness * metallic_roughness.g;
//        }
//#endif
//        pbr_input.material.metallic = metallic;
//        pbr_input.material.perceptual_roughness = perceptual_roughness;
//
//        // TODO: Split into diffuse/specular occlusion?
//        var occlusion: vec3<f32> = vec3(1.0);
//#ifdef VERTEX_UVS
//        if ((pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_OCCLUSION_TEXTURE_BIT) != 0u) {
//            occlusion = vec3(textureSampleBias(pbr_bindings::occlusion_texture, pbr_bindings::occlusion_sampler, uv, view.mip_bias).r);
//        }
//#endif
//#ifdef SCREEN_SPACE_AMBIENT_OCCLUSION
//        let ssao = textureLoad(screen_space_ambient_occlusion_texture, vec2<i32>(in.position.xy), 0i).r;
//        let ssao_multibounce = gtao_multibounce(ssao, pbr_input.material.base_color.rgb);
//        occlusion = min(occlusion, ssao_multibounce);
//#endif
//        pbr_input.occlusion = occlusion;
//
//        pbr_input.frag_coord = in.position;
//        pbr_input.world_position = in.world_position;
//
//        pbr_input.world_normal = pbr_functions::prepare_world_normal(
//            in.world_normal,
//            (pbr_bindings::material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u,
//            in.is_front,
//        );
//
//        pbr_input.is_orthographic = is_orthographic;
//
//#ifdef LOAD_PREPASS_NORMALS
//        pbr_input.N = bevy_pbr::prepass_utils::prepass_normal(in.position, 0u);
//#else
//        pbr_input.N = pbr_functions::apply_normal_mapping(
//            pbr_bindings::material.flags,
//            pbr_input.world_normal,
//#ifdef VERTEX_TANGENTS
//#ifdef STANDARDMATERIAL_NORMAL_MAP
//            in.world_tangent,
//#endif
//#endif
//#ifdef VERTEX_UVS
//            uv,
//#endif
//            view.mip_bias,
//        );
//#endif
//
//        pbr_input.V = V;
//        pbr_input.occlusion = occlusion;
//
//        pbr_input.flags = mesh.flags;
//
//        output_color = pbr_functions::pbr(pbr_input);
//    } else {
        output_color = pbr_functions::alpha_discard(pbr_bindings::material, output_color);
//    }

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
