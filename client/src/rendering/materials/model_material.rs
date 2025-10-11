use bevy::{
    asset::{load_internal_asset, weak_handle},
    math::{DVec3, Vec3A},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::{
        mesh::{
            MeshAabb, MeshTag, MeshVertexAttribute, MeshVertexBufferLayoutRef,
            VertexAttributeValues,
        },
        render_resource::*,
    },
};

use std::collections::HashMap;

use crate::{
    assets::BlockTextures,
    game_state::GameState,
    rendering::lighting::{Light, LightMap},
    world::Origin,
};

const MODEL_SHADER: Handle<Shader> = weak_handle!("5271e945-44f0-49e2-9ca1-50225dbb5565");

pub type ModelMaterial = ExtendedMaterial<StandardMaterial, ModelMaterialExtension>;

pub struct ModelMaterialPlugin;
impl Plugin for ModelMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelMaterials::default())
            .add_plugins(MaterialPlugin::<ModelMaterial> {
                shadows_enabled: false,
                prepass_enabled: true,
                ..default()
            })
            // Some weird schedule ordering here to avoid flickering. Chosen at random until it
            // worked.
            .add_systems(
                PostUpdate,
                (add_lighting, replace_standard_material).run_if(in_state(GameState::Playing)),
            )
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

fn add_lighting(
    mut commands: Commands,
    material_query: Query<Entity, (With<MeshMaterial3d<ModelMaterial>>, Without<MeshTag>)>,
) {
    for entity in material_query.iter() {
        commands.entity(entity).insert(MeshTag(0));
    }
}

fn replace_standard_material(
    mut commands: Commands,
    mut model_materials: ResMut<ModelMaterials>,
    material_query: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    standard_material_assets: Res<Assets<StandardMaterial>>,
    mut model_material_assets: ResMut<Assets<ModelMaterial>>,
    block_textures: Res<BlockTextures>,
) {
    for (entity, standard_handle) in material_query.iter() {
        let model_material = if let Some(model_material) = model_materials.get(standard_handle) {
            model_material.clone()
        } else {
            let standard_material = standard_material_assets.get(standard_handle).unwrap();
            model_material_assets.add(ExtendedMaterial {
                base: standard_material.clone(),
                extension: ModelMaterialExtension {
                    block_textures: block_textures.handle.clone(),
                },
            })
        };

        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<MeshMaterial3d<StandardMaterial>>();
        // MeshTag is used for per instance lighting
        entity_commands.insert((MeshMaterial3d(model_material), MeshTag(0)));
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ModelMaterialExtension {
    #[texture(31, dimension = "2d_array")]
    #[sampler(32)]
    pub block_textures: Handle<Image>,
}

impl ModelMaterialExtension {
    pub const ATTRIBUTE_BLOCK_TEXTURE_INDEX: MeshVertexAttribute = MeshVertexAttribute::new(
        "block texture index",
        //Mesh::FIRST_AVAILABLE_CUSTOM_ATTRIBUTE,
        8000,
        VertexFormat::Uint32,
    );
}

impl MaterialExtension for ModelMaterialExtension {
    fn vertex_shader() -> ShaderRef {
        MODEL_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        MODEL_SHADER.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(index) = layout
            .0
            .attribute_ids()
            .iter()
            .position(|id| *id == Self::ATTRIBUTE_BLOCK_TEXTURE_INDEX.at_shader_location(8).id)
        {
            let layout_attribute = layout.0.layout().attributes[index];

            descriptor.vertex.shader_defs.push("BLOCK_TEXTURE".into());
            descriptor
                .fragment
                .as_mut()
                .unwrap()
                .shader_defs
                .push("BLOCK_TEXTURE".into());
            descriptor.vertex.buffers[0]
                .attributes
                .push(VertexAttribute {
                    format: layout_attribute.format,
                    offset: layout_attribute.offset,
                    shader_location: 8,
                });
        }

        return Ok(());
    }
}
