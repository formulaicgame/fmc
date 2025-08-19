use bevy::{
    asset::load_internal_asset,
    math::{DVec3, Vec3A},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        mesh::{MeshAabb, MeshTag, MeshVertexBufferLayoutRef, VertexAttributeValues},
        render_resource::*,
    },
};

use std::collections::HashMap;

use crate::{
    game_state::GameState,
    rendering::lighting::{Light, LightMap},
    world::Origin,
};

const MODEL_SHADER: Handle<Shader> = Handle::weak_from_u128(34096891246294360);

pub type ModelMaterial = ExtendedMaterial<StandardMaterial, Extension>;

pub struct ModelMaterialPlugin;
impl Plugin for ModelMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelMaterials::default())
            .add_plugins(MaterialPlugin::<ModelMaterial> {
                shadows_enabled: false,
                prepass_enabled: true,
                ..default()
            })
            // Some weird schedule ordering here to avoid flickering when replacing the meshes.
            // Chosen at random until it worked.
            .add_systems(PostUpdate, replace_material)
            .add_systems(Last, update_light)
            .add_systems(OnEnter(GameState::Launcher), cleanup);

        load_internal_asset!(
            app,
            MODEL_SHADER,
            "../shaders/models.wgsl",
            Shader::from_wgsl
        );
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct ModelMaterials(HashMap<Handle<StandardMaterial>, Handle<ModelMaterial>>);

fn cleanup(mut materials: ResMut<ModelMaterials>) {
    materials.clear();
}

fn update_light(
    origin: Res<Origin>,
    light_map: Res<LightMap>,
    mut mesh_query: Query<
        (&GlobalTransform, &Mesh3d, &mut MeshTag),
        (
            With<MeshMaterial3d<ModelMaterial>>,
            Or<(
                Changed<GlobalTransform>,
                Added<MeshMaterial3d<ModelMaterial>>,
            )>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (transform, mesh_handle, mut mesh_tag) in mesh_query.iter_mut() {
        let transform = transform.compute_transform();
        let mesh = meshes.get_mut(mesh_handle).unwrap();
        let mut mesh_aabb = mesh.compute_aabb().unwrap();
        mesh_aabb.center *= Vec3A::from(transform.scale);
        mesh_aabb.half_extents *= Vec3A::from(transform.scale);

        let position = origin.to_global(transform.translation);

        // There's an assumption here that lighting is finsihed before this first runs that I don't
        // know if holds true.
        let mut new_light = Light(0);
        for (i, offset) in mesh_aabb.half_extents.to_array().into_iter().enumerate() {
            let mut offset_vec = DVec3::ZERO;
            offset_vec[i] = offset as f64;
            for direction in [-1.0, 1.0] {
                let position = (position + mesh_aabb.center.as_dvec3() + offset_vec * direction)
                    .floor()
                    .as_ivec3();
                if let Some(light) = light_map.get_light(position) {
                    if light.sunlight() > new_light.sunlight() {
                        new_light.set_sunlight(light.sunlight());
                    }
                    if light.artificial() > new_light.artificial() {
                        new_light.set_artificial(light.artificial());
                    }
                }
            }
        }

        mesh_tag.0 = new_light.0 as u32;
    }
}

fn replace_material(
    mut commands: Commands,
    mut model_materials: ResMut<ModelMaterials>,
    material_query: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    standard_material_assets: Res<Assets<StandardMaterial>>,
    mut model_material_assets: ResMut<Assets<ModelMaterial>>,
) {
    for (entity, standard_handle) in material_query.iter() {
        let model_material = if let Some(model_material) = model_materials.get(standard_handle) {
            model_material.clone()
        } else {
            let standard_material = standard_material_assets.get(standard_handle).unwrap();
            model_material_assets.add(ExtendedMaterial {
                base: standard_material.clone(),
                extension: Extension::default(),
            })
        };

        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<MeshMaterial3d<StandardMaterial>>();
        // MeshTag is used for per instance lighting
        entity_commands.insert((MeshMaterial3d(model_material), MeshTag(0)));
    }
}

#[derive(Default, Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Extension {
    #[uniform(100)]
    _dummy: u32,
}

impl MaterialExtension for Extension {
    fn vertex_shader() -> ShaderRef {
        MODEL_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        MODEL_SHADER.into()
    }
}
