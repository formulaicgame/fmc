use bevy::{
    asset::load_internal_asset,
    math::{DVec3, Vec3A},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        mesh::{MeshAabb, MeshVertexBufferLayoutRef, VertexAttributeValues},
        render_resource::*,
    },
};

use crate::{
    rendering::lighting::{Light, LightMap},
    world::Origin,
};

use super::ATTRIBUTE_PACKED_BITS_0;

const PBR_MESH_SHADER: Handle<Shader> = Handle::weak_from_u128(34096891246294360);
const PBR_FRAGMENT_SHADER: Handle<Shader> = Handle::weak_from_u128(569708293840967);

pub struct PbrMaterialPlugin;
impl Plugin for PbrMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            MaterialPlugin::<ExtendedMaterial<StandardMaterial, PbrLightExtension>> {
                shadows_enabled: false,
                prepass_enabled: false,
                ..default()
            },
        )
        // Some weird schedule ordering here to avoid flickering when replacing the meshes.
        // Chosen at random until it worked.
        .add_systems(PostUpdate, replace_material_and_mesh)
        .add_systems(Last, update_light);

        load_internal_asset!(
            app,
            PBR_MESH_SHADER,
            "../shaders/pbr_mesh.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            PBR_FRAGMENT_SHADER,
            "../shaders/pbr.wgsl",
            Shader::from_wgsl
        );
    }
}

fn update_light(
    origin: Res<Origin>,
    light_map: Res<LightMap>,
    mesh_query: Query<
        (&GlobalTransform, &Mesh3d),
        (
            With<MeshMaterial3d<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
            Or<(
                Changed<GlobalTransform>,
                Added<MeshMaterial3d<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
            )>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (transform, mesh_handle) in mesh_query.iter() {
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

        if let Some(light_attr) = mesh.attribute(ATTRIBUTE_PACKED_BITS_0) {
            let light_attr = match light_attr {
                VertexAttributeValues::Uint32(l) => l,
                _ => unreachable!(),
            };
            if let Some(old_light) = light_attr.get(0) {
                if new_light == Light(*old_light as u8) {
                    continue;
                }
            }
        }

        let len = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(positions) => positions.len(),
            _ => unreachable!(),
        };

        let new_light = vec![new_light.0 as u32; len];
        mesh.insert_attribute(ATTRIBUTE_PACKED_BITS_0, new_light);
    }
}

// Gltf's automatically use StandardMaterial, and their meshes are shared between all instances of
// the object. Since the light level is embedded in the mesh, a new mesh needs to be inserted for
// each as well as replacing the material it uses.
fn replace_material_and_mesh(
    mut commands: Commands,
    material_query: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>, &Mesh3d),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    standard_materials: Res<Assets<StandardMaterial>>,
    mut pbr_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, PbrLightExtension>>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, standard_handle, mesh_handle) in material_query.iter() {
        let standard_material = standard_materials.get(standard_handle).unwrap();
        let extension_handle = MeshMaterial3d(pbr_materials.add(ExtendedMaterial {
            base: standard_material.clone(),
            extension: PbrLightExtension::default(),
        }));
        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<MeshMaterial3d<StandardMaterial>>();
        entity_commands.insert(extension_handle);
        let mesh = meshes.get(mesh_handle).unwrap().clone();
        // Copy the mesh, light is baked into each individual mesh
        entity_commands.insert(Mesh3d(meshes.add(mesh)));
    }
}

#[derive(Default, Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PbrLightExtension {
    // XXX: This is a useless variable to satisfy the AsBindGroup requirement. Ripped from example
    #[uniform(100)]
    _dummy: u32,
}

impl MaterialExtension for PbrLightExtension {
    fn vertex_shader() -> ShaderRef {
        PBR_MESH_SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        PBR_FRAGMENT_SHADER.into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        //let vertex_layout = layout
        //    .get_layout(&[
        //        Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
        //        ATTRIBUTE_PACKED_BITS_0.at_shader_location(2),
        //    ])
        //    .unwrap();
        // I'll probably get bit in the ass for doing this, but I don't want to keep it in sync
        // with changes to StandardMaterial. I have no idea what side effects this might cause, I
        // just did kinda what the bevy code does.
        let index = layout
            .0
            .attribute_ids()
            .iter()
            .position(|id| *id == ATTRIBUTE_PACKED_BITS_0.at_shader_location(8).id)
            .unwrap();
        let layout_attribute = layout.0.layout().attributes[index];
        descriptor.vertex.buffers[0]
            .attributes
            .push(VertexAttribute {
                format: layout_attribute.format,
                offset: layout_attribute.offset,
                shader_location: ATTRIBUTE_PACKED_BITS_0
                    .at_shader_location(8)
                    .shader_location,
            });
        Ok(())
    }
}
