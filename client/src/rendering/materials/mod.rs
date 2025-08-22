use bevy::{
    prelude::*,
    render::{mesh::MeshVertexAttribute, render_resource::VertexFormat},
};

mod block_material;
mod model_material;
mod particle_material;
mod sky_material;

pub use block_material::BlockMaterial;
pub use model_material::{ModelMaterial, ModelMaterialExtension};
pub use particle_material::ParticleMaterial;
pub use sky_material::SkyMaterial;

pub struct MaterialsPlugin;
impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(block_material::BlockMaterialPlugin)
            .add_plugins(sky_material::SkyMaterialPlugin)
            .add_plugins(particle_material::ParticleMaterialPlugin)
            .add_plugins(model_material::ModelMaterialPlugin);
    }
}
