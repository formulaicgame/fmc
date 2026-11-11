#import bevy_pbr::mesh_view_bindings::{
    view,
    lights,
    fog
}

#import bevy_core_pipeline::tonemapping::{
    screen_space_dither,
    tone_mapping
} 

#import bevy_pbr::forward_io::VertexOutput

#import bevy_render::maths::{PI, HALF_PI, powsafe}

#import fmc::lighting::{sun_height, brightness, fog_color, VOID_COLOR}

const HORIZON = -0.05;

struct SkyMaterialUniform {
    is_sun: u32,
    is_moon: u32,
    is_star: u32,
    sun_angle: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: SkyMaterialUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var texture_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    var output_color: vec4<f32>;

#ifdef VERTEX_UVS_A
    output_color = textureSample(texture, texture_sampler, in.uv);
#endif

    // Move fragment position to be relative to the origin. All calculations are 
    // done assuming a unit circle.
    let position = normalize(in.world_position.xyz - view.world_position.xyz);

    let sky_color = sky(position);

    if material.is_star != 0 {
        let starlight = mix(vec4(1.0), sky_color, smoothstep(-0.5, -0.1, sun_height()));
        // Hide stars inside glow of the moon
        output_color = mix(sky_color, starlight, step(celestial_glow(position).a, 0.0));
    } else if material.is_sun != 0 {
        output_color = output_color * 1.5 * sun_color();
    } else if material.is_moon != 0 {
        // Tints the moon slightly blue
        let emission = vec4(1.0, 1.0, 1.8, 1.0);
        output_color = output_color * emission;
        // Fade the moon as it sets/rises
        output_color = mix(sky_color, output_color, moon_visibility());
    } else {
        output_color = sky_color;
    }

    // Filter out stars/moon/sun when below horizon
    output_color = mix(sky_color, output_color, step(HORIZON, position.y));

#ifdef TONEMAP_IN_SHADER
    // output_color = tone_mapping(output_color, view.color_grading);
#ifdef DEBAND_DITHER
    var output_rgb = output_color.rgb;
    output_rgb = powsafe(output_rgb, 1.0 / 2.2);
    output_rgb += screen_space_dither(in.position.xy);
    // This conversion back to linear space is required because our output texture format is
    // SRGB; the GPU will assume our output is linear and will apply an SRGB conversion.
    output_rgb = powsafe(output_rgb, 2.2);
    output_color = vec4(output_rgb, output_color.a);
#endif
#endif
//#ifdef PREMULTIPLY_ALPHA
//    output_color = premultiply_alpha(material.flags, output_color);
//#endif
    return output_color;
} 

fn sky(position: vec3<f32>) -> vec4<f32> {
    let day_color = vec4(0.1, 0.4, 1.0, 1.0);
    // Important that the alpha is zero here so that the day color will overrule
    // the moon color when blending, but the night color will not.
    let night_color = vec4(VOID_COLOR, 0.0);

    var sky_color = mix(night_color, day_color, brightness());

    let void_color = vec4(fog_color(fog).rgb, sky_color.a);
    sky_color = mix(void_color, sky_color, smoothstep(0.01, 0.50, position.y));

    sky_color = blend_colors(sky_color, twilight(position));

    // Apply glow from sun/moon
    sky_color = mix(sky_color, blend_colors(sky_color, celestial_glow(position)), step(HORIZON, position.y));

    return sky_color;
}

// sun/moon glow
fn celestial_glow(position: vec3<f32>) -> vec4<f32> {
    let sun_direction = normalize(vec3(cos(material.sun_angle), sin(material.sun_angle), 0.0));
    let sun_dot = dot(position, sun_direction);
    let moon_dot = dot(position, -sun_direction);

    let sun_glow = vec4(1.0, 1.0, 0.0, 0.25) * sun_color() * smoothstep(-0.1, 0.0, sun_height());
    let moon_glow = vec4(0.4, 0.4, 0.7, 0.04) * moon_visibility();
    let color = select(sun_glow, moon_glow, vec4(moon_dot > sun_dot));

    return color * smoothstep(0.96, 1.0, max(sun_dot, moon_dot));
}

fn twilight(position: vec3<f32>) -> vec4<f32> {
    let max_color = vec4(0.95, 0.25, 0.08, 1.0);
    // Important that the alpha is zero here so that the day color will overrule
    // the moon color when blending, but the night color will not.
    let no_color = vec4(0.0);

    // TODO: Fade point chosen to taste, they don't provide much control. You
    // set the below low so it will linger long, but now it shows too early for
    // the sunrise and so for the above color too.
    //
    // The midpoint is set so the color will be at its max when the entire sun
    // is in view but is as close to the horizon as possible. 
    let midpoint = 0.0;
    let above_color = mix(max_color, no_color, smoothstep(midpoint, 0.15, sun_height()));
    let below_color = mix(no_color, max_color, linearstep(-0.2, midpoint, sun_height()));
    var color = min(above_color, below_color);

    // TODO: lock to sun position for stricter follow when below horizon?
    // let center = min(sun_height(), HORIZON);

    // The color is reduced the farther the frament is from the horizon vertically
    let height = 0.35; // Max height the color can reach
    color.a = color.a - max_color.a / height * abs(position.y - HORIZON);
    color.a = color.a - (dot(
        vec2(
            // Invert the cosine because we want it to decrease where the sun isn't
            sign(-cos(material.sun_angle)),
            0.0
        ),
        normalize(position.xz)) + 1.0);

    // Color the fake surface a little as a reflection
    color = mix(color * 0.6, color, step(HORIZON, position.y));

    color.a = clamp(color.a, 0.0, 1.0);
    return color;
}

fn sun_color() -> vec4<f32> {
    let warm = vec4(1.0, 0.51, 0.21, 1.0);
    return mix(warm, vec4(1.0), smoothstep(0.0, 0.20, sun_height()));
}

fn moon_visibility() -> f32 {
    return 1.0 - smoothstep(-0.15, -0.05, sun_height());
}

fn linearstep(low: f32, high: f32, x: f32) -> f32 {
    return saturate((x - low) / (high - low));
}

fn blend_colors(background: vec4<f32>, foreground: vec4<f32>) -> vec4<f32> {
    // XXX: No idea how this premultiply stuff works, but it looks much better.
    let premul_background = background.rgb * background.a;
    let premul_foreground = foreground.rgb * foreground.a;
    let alpha = foreground.a + (1 - foreground.a) * background.a;
    let color = (foreground.rgb * foreground.a + background.rgb * (1 - foreground.a));
    return vec4(color, alpha);
}

