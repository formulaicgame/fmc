use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{mesh::MeshVertexBufferLayout, render_resource::*},
};

#[derive(Asset, AsBindGroup, Debug, Clone, TypePath)]
//#[bind_group_data(BlockMaterialKey)]
#[uniform(0, SkyMaterialUniform)]
pub struct SkyMaterial {
    pub mie_k_coefficient: Vec4,
    pub primaries: Vec4,
    pub sun_position: Vec4,
    pub depolarization_factor: f32,
    pub luminance: f32,
    pub mie_coefficient: f32,
    pub mie_directional_g: f32,
    pub mie_v: f32,
    pub mie_zenith_length: f32,
    pub num_molecules: f32,
    pub rayleigh: f32,
    pub rayleigh_zenith_length: f32,
    pub refractive_index: f32,
    pub sun_angular_diameter_degrees: f32,
    pub sun_intensity_factor: f32,
    pub sun_intensity_falloff_steepness: f32,
    pub tonemap_weighting: f32,
    pub turbidity: f32,
}

// Defaults to the red sunset preset from https://tw1ddle.github.io/Sky-Shader/
impl Default for SkyMaterial {
    fn default() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(6.8e-7, 5.5e-7, 4.5e-7, 0.0),
            sun_position: Vec4::ZERO,
            depolarization_factor: 0.02,
            luminance: 1.00,
            mie_coefficient: 0.005,
            mie_directional_g: 0.82,
            mie_v: 3.936,
            mie_zenith_length: 34000.0,
            num_molecules: 2.542e25,
            rayleigh: 2.28,
            rayleigh_zenith_length: 8400.0,
            refractive_index: 1.00029,
            sun_angular_diameter_degrees: 0.00933,
            sun_intensity_factor: 1000.0,
            sun_intensity_falloff_steepness: 1.5,
            tonemap_weighting: 9.50,
            turbidity: 4.7,
        }
    }
}

impl SkyMaterial {
    /// inclination in [-pi/2, pi/2], azimuth in [-pi, pi]
    pub fn stellar_dawn() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(6.8e-7, 5.5e-7, 4.5e-7, 0.0),
            depolarization_factor: 0.067,
            luminance: 1.0,
            mie_coefficient: 0.00335,
            mie_directional_g: 0.787,
            mie_v: 4.012,
            mie_zenith_length: 500.0,
            num_molecules: 2.542e25,
            rayleigh_zenith_length: 615.0,
            rayleigh: 1.00,
            refractive_index: 1.000317,
            sun_angular_diameter_degrees: 0.00758,
            sun_intensity_factor: 1111.0,
            sun_intensity_falloff_steepness: 0.98,
            tonemap_weighting: 9.50,
            turbidity: 1.25,
            ..Default::default()
        }
    }

    pub fn red_sunset() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(6.8e-7, 5.5e-7, 4.5e-7, 0.0),
            turbidity: 4.7,
            rayleigh: 2.28,
            mie_coefficient: 0.005,
            mie_directional_g: 0.82,
            luminance: 1.00,
            refractive_index: 1.00029,
            num_molecules: 2.542e25,
            depolarization_factor: 0.02,
            rayleigh_zenith_length: 8400.0,
            mie_v: 3.936,
            mie_zenith_length: 34000.0,
            sun_intensity_factor: 1000.0,
            sun_intensity_falloff_steepness: 1.5,
            sun_angular_diameter_degrees: 0.00933,
            tonemap_weighting: 9.50,
            ..Default::default()
        }
    }

    pub fn alien_day() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(6.8e-7, 5.5e-7, 4.5e-7, 0.0),
            turbidity: 12.575,
            rayleigh: 5.75,
            mie_coefficient: 0.0074,
            mie_directional_g: 0.468,
            luminance: 1.00,
            refractive_index: 1.000128,
            num_molecules: 2.542e25,
            depolarization_factor: 0.137,
            rayleigh_zenith_length: 3795.0,
            mie_v: 4.007,
            mie_zenith_length: 7100.0,
            sun_intensity_factor: 1024.0,
            sun_intensity_falloff_steepness: 1.4,
            sun_angular_diameter_degrees: 0.006,
            tonemap_weighting: 9.50,
            ..Default::default()
        }
    }

    pub fn blue_dusk() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(6.8e-7, 5.5e-7, 4.5e-7, 0.0),
            turbidity: 2.5,
            rayleigh: 2.295,
            mie_coefficient: 0.011475,
            mie_directional_g: 0.814,
            luminance: 1.00,
            refractive_index: 1.000262,
            num_molecules: 2.542e25,
            depolarization_factor: 0.095,
            rayleigh_zenith_length: 540.0,
            mie_v: 3.979,
            mie_zenith_length: 1000.0,
            sun_intensity_factor: 1151.0,
            sun_intensity_falloff_steepness: 1.22,
            sun_angular_diameter_degrees: 0.00639,
            tonemap_weighting: 9.50,
            ..Default::default()
        }
    }

    pub fn purple_dusk() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(7.5e-7, 4.5e-7, 5.1e-7, 0.0),
            turbidity: 3.6,
            rayleigh: 2.26,
            mie_coefficient: 0.005,
            mie_directional_g: 0.822,
            luminance: 1.00,
            refractive_index: 1.000294,
            num_molecules: 2.542e25,
            depolarization_factor: 0.068,
            rayleigh_zenith_length: 12045.0,
            mie_v: 3.976,
            mie_zenith_length: 34000.0,
            sun_intensity_factor: 1631.0,
            sun_intensity_falloff_steepness: 1.5,
            sun_angular_diameter_degrees: 0.00933,
            tonemap_weighting: 9.50,
            ..Default::default()
        }
    }

    pub fn blood_sky() -> Self {
        Self {
            mie_k_coefficient: Vec4::new(0.686, 0.678, 0.666, 0.0),
            primaries: Vec4::new(7.929e-7, 3.766e-7, 3.172e-7, 0.0),
            turbidity: 4.75,
            rayleigh: 6.77,
            mie_coefficient: 0.0191,
            mie_directional_g: 0.793,
            luminance: 1.1735,
            refractive_index: 1.000633,
            num_molecules: 2.542e25,
            depolarization_factor: 0.01,
            rayleigh_zenith_length: 1425.0,
            mie_v: 4.042,
            mie_zenith_length: 1600.0,
            sun_intensity_factor: 2069.0,
            sun_intensity_falloff_steepness: 2.26,
            sun_angular_diameter_degrees: 0.01487,
            tonemap_weighting: 9.50,
            ..Default::default()
        }
    }
}

#[derive(Clone, ShaderType)]
pub struct SkyMaterialUniform {
    pub mie_k_coefficient: Vec4,
    pub primaries: Vec4,
    pub sun_position: Vec4,
    pub depolarization_factor: f32,
    pub luminance: f32,
    pub mie_coefficient: f32,
    pub mie_directional_g: f32,
    pub mie_v: f32,
    pub mie_zenith_length: f32,
    pub num_molecules: f32,
    pub rayleigh: f32,
    pub rayleigh_zenith_length: f32,
    pub refractive_index: f32,
    pub sun_angular_diameter_degrees: f32,
    pub sun_intensity_factor: f32,
    pub sun_intensity_falloff_steepness: f32,
    pub tonemap_weighting: f32,
    pub turbidity: f32,
}

impl From<&SkyMaterial> for SkyMaterialUniform {
    fn from(material: &SkyMaterial) -> Self {
        Self {
            mie_k_coefficient: material.mie_k_coefficient,
            primaries: material.primaries,
            sun_position: material.sun_position,
            depolarization_factor: material.depolarization_factor,
            luminance: material.luminance,
            mie_coefficient: material.mie_coefficient,
            mie_directional_g: material.mie_directional_g,
            mie_v: material.mie_v,
            mie_zenith_length: material.mie_zenith_length,
            num_molecules: material.num_molecules,
            rayleigh: material.rayleigh,
            rayleigh_zenith_length: material.rayleigh_zenith_length,
            refractive_index: material.refractive_index,
            sun_angular_diameter_degrees: material.sun_angular_diameter_degrees,
            sun_intensity_factor: material.sun_intensity_factor,
            sun_intensity_falloff_steepness: material.sun_intensity_falloff_steepness,
            tonemap_weighting: material.tonemap_weighting,
            turbidity: material.turbidity,
        }
    }
}

impl Material for SkyMaterial {
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayout,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Flip to see the inside of the sphere
        descriptor.primitive.front_face = FrontFace::Cw;
        Ok(())
    }

    fn fragment_shader() -> ShaderRef {
        "src/rendering/shaders/physical_sky.wgsl".into()
    }
}
