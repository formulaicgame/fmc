#import bevy_pbr::mesh_view_bindings::{
    view,
    fog
}
#import bevy_pbr::pbr_functions::apply_fog
#import bevy_pbr::forward_io::VertexOutput

struct SkyMaterialUniform {
    mieKCoefficient: vec4<f32>,
    primaries: vec4<f32>,
    sunPosition: vec4<f32>,
    depolarizationFactor: f32,
    luminance: f32,
    mieCoefficient: f32,
    mieDirectionalG: f32,
    mieV: f32,
    mieZenithLength: f32,
    numMolecules: f32,
    rayleigh: f32,
    rayleighZenithLength: f32,
    refractiveIndex: f32,
    sunAngularDiameterDegrees: f32,
    sunIntensityFactor: f32,
    sunIntensityFalloffSteepness: f32,
    tonemapWeighting: f32,
    turbidity: f32,
};

@group(2) @binding(0)
var<uniform> sky_material_uniform: SkyMaterialUniform;

const PI: f32 = 3.1415927;
const UP: vec3<f32> = vec3<f32>(0., 1., 0.);
fn totalRayleigh(lambda: vec3<f32>) -> vec3<f32> {
    return 8. * pow(PI, 3.) * pow(pow(sky_material_uniform.refractiveIndex, 2.) - 1., 2.) * (6. + 3. * sky_material_uniform.depolarizationFactor) / (3. * sky_material_uniform.numMolecules * pow(lambda, vec3<f32>(4.)) * (6. - 7. * sky_material_uniform.depolarizationFactor));
} 

fn totalMie(lambda: vec3<f32>, K: vec3<f32>, T: f32) -> vec3<f32> {
    let c: f32 = 0.2 * T * 0.00000000000000001;
    return 0.434 * c * PI * pow(2. * PI / lambda, vec3<f32>(sky_material_uniform.mieV - 2.)) * K;
} 

fn rayleighPhase(cosTheta: f32) -> f32 {
    return 3. / (16. * PI) * (1. + pow(cosTheta, 2.));
} 

fn henyeyGreensteinPhase(cosTheta: f32, g: f32) -> f32 {
    return 1. / (4. * PI) * ((1. - pow(g, 2.)) / pow(1. - 2. * g * cosTheta + pow(g, 2.), 1.5));
} 

fn sunIntensity(zenithAngleCos: f32) -> f32 {
    let cutoffAngle: f32 = PI / 1.95;
    return sky_material_uniform.sunIntensityFactor * max(0., 1. - exp(-((cutoffAngle - acos(zenithAngleCos)) / sky_material_uniform.sunIntensityFalloffSteepness)));
} 

const A: f32 = 0.15;
const B: f32 = 0.5;
const C: f32 = 0.1;
const D: f32 = 0.2;
const E: f32 = 0.02;
const F: f32 = 0.3;
fn Uncharted2Tonemap(W: vec3<f32>) -> vec3<f32> {
    return (W * (A * W + C * B) + D * E) / (W * (A * W + B) + D * F) - E / F;
} 

struct FragmentInput {
    @builtin(position) world_position: vec4<f32>,
}

@fragment
fn fragment(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    // Rayleigh coefficient
    let sunfade: f32 = 1. - clamp(1. - exp(sky_material_uniform.sunPosition.y / 450000.), 0., 1.);
    let rayleighCoefficient: f32 = sky_material_uniform.rayleigh - 1. * (1. - sunfade);
    let betaR: vec3<f32> = totalRayleigh(sky_material_uniform.primaries.rgb) * rayleighCoefficient;

    // Mie coefficient
    let betaM: vec3<f32> = totalMie(sky_material_uniform.primaries.rgb, sky_material_uniform.mieKCoefficient.rgb, sky_material_uniform.turbidity) * sky_material_uniform.mieCoefficient;

    // Optical length, cutoff angle at 90 to avoid singularity
    let zenithAngle: f32 = acos(max(0., dot(UP, normalize(in.world_position.xyz - view.world_position))));
    let denom: f32 = cos(zenithAngle) + 0.15 * pow(93.885 - zenithAngle * 180. / PI, -1.253);
    let sR: f32 = sky_material_uniform.rayleighZenithLength / denom;
    let sM: f32 = sky_material_uniform.mieZenithLength / denom;

    // Combined extinction factor
    let Fex: vec3<f32> = exp(-(betaR * sR + betaM * sM));

    // In-scattering
    let sunDirection: vec3<f32> = normalize(sky_material_uniform.sunPosition.xyz);
    let cosTheta: f32 = dot(normalize(in.world_position.xyz - view.world_position), sunDirection);
    let betaRTheta: vec3<f32> = betaR * rayleighPhase(cosTheta * 0.5 + 0.5);
    let betaMTheta: vec3<f32> = betaM * henyeyGreensteinPhase(cosTheta, sky_material_uniform.mieDirectionalG);
    let sunE: f32 = sunIntensity(dot(sunDirection, UP));
    var Lin: vec3<f32> = pow(sunE * ((betaRTheta + betaMTheta) / (betaR + betaM)) * (1. - Fex), vec3<f32>(1.5));
    Lin = Lin * (mix(vec3<f32>(1.), pow(sunE * ((betaRTheta + betaMTheta) / (betaR + betaM)) * Fex, vec3<f32>(0.5)), clamp(pow(1. - dot(UP, sunDirection), 5.), 0., 1.)));

    // Composition + solar disc
    let sunAngularDiameterCos: f32 = cos(sky_material_uniform.sunAngularDiameterDegrees);
    let sundisk: f32 = smoothstep(sunAngularDiameterCos, sunAngularDiameterCos + 0.00002, cosTheta);
    var L0: vec3<f32> = vec3<f32>(0.1) * Fex;
    L0 = L0 + (sunE * 19000. * Fex * sundisk);
    var texColor: vec3<f32> = Lin + L0;
    texColor = texColor * (0.04);
    texColor = texColor + (vec3<f32>(0., 0.001, 0.0025) * 0.3);

    // Tonemapping
    let whiteScale: vec3<f32> = 1. / Uncharted2Tonemap(vec3<f32>(sky_material_uniform.tonemapWeighting));
    let curr: vec3<f32> = Uncharted2Tonemap(log2(2. / pow(sky_material_uniform.luminance, 4.)) * texColor);
    let color: vec3<f32> = curr * whiteScale;
    let retColor: vec3<f32> = pow(color, vec3<f32>(1. / (1.2 + 1.2 * sunfade)));

    let output_color = apply_fog(fog, vec4(retColor, 1.0), in.world_position.xyz, view.world_position.xyz);
    return output_color;
} 

