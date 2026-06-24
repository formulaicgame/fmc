use bevy::{
    asset::{Handle, load_internal_asset, uuid_handle},
    image::Image,
    mesh::MeshTag,
    prelude::*,
    reflect::TypePath,
    render::render_resource::*,
    shader::ShaderRef,
};

use crate::{rendering::lighting::LightMap, world::Origin};

const PARTICLE_SHADER: Handle<Shader> = uuid_handle!("d0c4577a-75e0-4794-9355-eb4b7a8a2036");

pub struct ParticleMaterialPlugin;
impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ParticleMaterial> {
            shadows_enabled: false,
            prepass_enabled: false,
            ..default()
        })
        .add_systems(Update, update_lighting);

        load_internal_asset!(
            app,
            PARTICLE_SHADER,
            "../shaders/particles.wgsl",
            Shader::from_wgsl
        );
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[uniform(0, ParticleMaterialUniform)]
pub struct ParticleMaterial {
    #[texture(1, dimension = "2d_array")]
    #[sampler(2)]
    pub texture: Handle<Image>,
    pub base_color: Srgba,
    // Min and max lifetime of the particle. The exact lifetime is sampled in the shader using the
    // mesh tag as seed, ensuring it's the same as the entity lifetime.
    pub lifetime: Vec2,
    // Samples a smaller section of the texture for each particle.
    pub random_uv: UVec2,
    pub spawn_time: f32,
}

impl Material for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        PARTICLE_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        PARTICLE_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(0.5)
    }
}

#[derive(Clone, Default, ShaderType)]
struct ParticleMaterialUniform {
    base_color: Vec4,
    lifetime: Vec2,
    random_uv: UVec2,
    spawn_time: f32,
}

impl From<&ParticleMaterial> for ParticleMaterialUniform {
    fn from(material: &ParticleMaterial) -> Self {
        Self {
            base_color: material.base_color.to_vec4(),
            lifetime: material.lifetime,
            random_uv: material.random_uv,
            spawn_time: material.spawn_time,
        }
    }
}

fn update_lighting(
    origin: Res<Origin>,
    light_map: Res<LightMap>,
    mut particles: Query<
        (&GlobalTransform, &mut MeshTag),
        (
            With<MeshMaterial3d<ParticleMaterial>>,
            Changed<GlobalTransform>,
        ),
    >,
) {
    for (transform, mut tag) in particles.iter_mut() {
        let position = origin.to_global(transform.translation()).floor().as_ivec3();
        let Some(light) = light_map.get_light(position) else {
            continue;
        };

        tag.0 &= !0xFF;
        tag.0 |= light.0 as u32;
    }
}
