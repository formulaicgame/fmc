use bevy::{
    asset::{load_internal_asset, Handle},
    prelude::*,
    reflect::TypePath,
    render::{render_resource::*, texture::Image},
};

use crate::{rendering::lighting::LightMap, world::Origin};

pub struct ParticleMaterialPlugin;
impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ParticleMaterial> {
            shadows_enabled: false,
            prepass_enabled: false,
            ..default()
        })
        .add_systems(Update, update_lighting);
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, ParticleMaterialUniform)]
pub struct ParticleMaterial {
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    /// Set to true if the particle is textured by a block texture.
    /// This results in the uvs being randomly generated, and the particle showing between 2 and 4
    /// pixels of the texture in each direction.
    pub block_texture: bool,
    pub base_color: Srgba,
}

impl Material for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        "src/rendering/shaders/particles.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "src/rendering/shaders/particles.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(0.5)
    }
}

#[derive(Clone, Default, ShaderType)]
struct ParticleMaterialUniform {
    // 0 if false, 1 if true
    block_texture: u32,
    base_color: Vec4,
}

impl From<&ParticleMaterial> for ParticleMaterialUniform {
    fn from(material: &ParticleMaterial) -> Self {
        Self {
            block_texture: material.block_texture as u32,
            base_color: material.base_color.to_vec4(),
        }
    }
}

fn update_lighting(
    origin: Res<Origin>,
    light_map: Res<LightMap>,
    ambient_light: Res<AmbientLight>,
    mut meshes: ResMut<Assets<Mesh>>,
    particles: Query<
        (&GlobalTransform, &Handle<Mesh>),
        (With<Handle<ParticleMaterial>>, Changed<GlobalTransform>),
    >,
) {
    for (transform, mesh_handle) in particles.iter() {
        let Some(mesh) = meshes.get_mut(mesh_handle) else {
            continue;
        };

        let position = origin.to_global(transform.translation()).as_ivec3();
        let Some(light) = light_map.get_light(position) else {
            continue;
        };

        let mut light = if light.sunlight() >= light.artificial() {
            0.8f32.powi(15 - light.sunlight() as i32) * ambient_light.brightness
        } else {
            0.8f32.powi(15 - light.artificial() as i32)
        };

        // This makes the particles darker to increase contrast
        light *= 0.7;

        let color = Vec4::new(light, light, light, 1.0);

        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![color; 4]);
    }
}
