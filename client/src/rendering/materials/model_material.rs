use bevy::{
    app::SceneSpawnerSystems,
    asset::{load_internal_asset, uuid_handle},
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::*,
    shader::ShaderRef,
};

use std::collections::HashMap;

use crate::{
    assets::BlockTextures, game_state::GameState, rendering::light_texture::LIGHT_TEXTURE,
    world::Origin,
};

const MODEL_SHADER: Handle<Shader> = uuid_handle!("5271e945-44f0-49e2-9ca1-50225dbb5565");

pub type ModelMaterial = ExtendedMaterial<StandardMaterial, ModelMaterialExtension>;

pub struct ModelMaterialPlugin;
impl Plugin for ModelMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ModelMaterials::default())
            .add_plugins(MaterialPlugin::<ModelMaterial>::default())
            .add_systems(
                SpawnScene,
                replace_standard_material
                    .after(SceneSpawnerSystems::WorldInstanceSpawn)
                    .run_if(in_state(GameState::Playing)),
            )
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

fn replace_standard_material(
    mut commands: Commands,
    model_materials: Res<ModelMaterials>,
    origin: Res<Origin>,
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
                    light_texture: LIGHT_TEXTURE,
                    origin: origin.0,
                },
            })
        };

        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<MeshMaterial3d<StandardMaterial>>();
        entity_commands.insert(MeshMaterial3d(model_material));
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ModelMaterialExtension {
    #[texture(31, dimension = "2d_array")]
    #[sampler(32)]
    pub block_textures: Handle<Image>,

    /// 3d texture holding the light level of every block within render distance. Sampled by world
    /// position, see the block material for details.
    #[texture(33, dimension = "3d")]
    #[sampler(34)]
    pub light_texture: Handle<Image>,

    #[uniform(35)]
    pub origin: IVec3,
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
    fn enable_shadows() -> bool {
        false
    }

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
