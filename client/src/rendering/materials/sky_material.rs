use bevy::{
    asset::load_internal_asset,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::{mesh::MeshVertexBufferLayoutRef, render_resource::*},
};

const SKY_SHADER: Handle<Shader> = Handle::weak_from_u128(1708015959337029744);

pub struct SkyMaterialPlugin;
impl Plugin for SkyMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial> {
            shadows_enabled: false,
            prepass_enabled: false,
            ..default()
        });

        load_internal_asset!(app, SKY_SHADER, "../shaders/sky.wgsl", Shader::from_wgsl);
    }
}

#[derive(Asset, AsBindGroup, Debug, Clone, TypePath, Default)]
#[uniform(0, SkyMaterialUniform)]
pub struct SkyMaterial {
    /// Texture to use for sun/moon/night sky
    #[texture(1)]
    #[sampler(2)]
    texture: Option<Handle<Image>>,
    // If this is the skybox, this is set
    pub sun_angle: f32,
    is_sun: bool,
    is_moon: bool,
    is_star: bool,
}

impl SkyMaterial {
    pub fn sun(texture: Handle<Image>) -> Self {
        Self {
            texture: Some(texture),
            is_sun: true,
            ..default()
        }
    }

    pub fn moon(texture: Handle<Image>) -> Self {
        Self {
            texture: Some(texture),
            is_moon: true,
            ..default()
        }
    }

    pub fn skybox() -> Self {
        Self::default()
    }

    pub fn star() -> Self {
        Self {
            is_star: true,
            ..default()
        }
    }
}

// The same material is used for both the skybox and the sun/moon so we have to distinguish them.
#[derive(Clone, ShaderType)]
pub struct SkyMaterialUniform {
    // above 0 if true, 0 if false
    is_sun: u32,
    is_moon: u32,
    is_star: u32,
    sun_angle: f32,
}

impl From<&SkyMaterial> for SkyMaterialUniform {
    fn from(material: &SkyMaterial) -> Self {
        Self {
            is_sun: material.is_sun as u32,
            is_moon: material.is_moon as u32,
            is_star: material.is_star as u32,
            sun_angle: material.sun_angle,
        }
    }
}

impl Material for SkyMaterial {
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Flip to see the inside of the sphere
        descriptor.primitive.front_face = FrontFace::Cw;
        Ok(())
    }

    fn fragment_shader() -> ShaderRef {
        SKY_SHADER.into()
    }
}
