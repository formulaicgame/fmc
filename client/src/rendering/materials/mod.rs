use bevy::prelude::*;

mod block_material;
mod model_material;
mod particle_material;
mod sky_material;

pub use block_material::BlockMaterial;
pub use model_material::{ModelMaterial, ModelMaterialExtension};
pub use particle_material::ParticleMaterial;
pub use sky_material::SkyMaterial;

use crate::world::Origin;

pub struct MaterialsPlugin;
impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(block_material::BlockMaterialPlugin)
            .add_plugins(sky_material::SkyMaterialPlugin)
            .add_plugins(particle_material::ParticleMaterialPlugin)
            .add_plugins(model_material::ModelMaterialPlugin);
        app.add_systems(
            PostUpdate,
            update_material_origin
                .after(crate::world::update_origin)
                .run_if(resource_changed::<Origin>),
        );
    }
}

fn update_material_origin(
    origin: Res<Origin>,
    mut block_materials: ResMut<Assets<BlockMaterial>>,
    mut model_materials: ResMut<Assets<ModelMaterial>>,
    mut particle_materials: ResMut<Assets<ParticleMaterial>>,
) {
    for (_, material) in block_materials.iter_mut() {
        material.origin = origin.0;
    }

    for (_, material) in model_materials.iter_mut() {
        material.extension.origin = origin.0;
    }

    for (_, material) in particle_materials.iter_mut() {
        material.origin = origin.0;
    }
}
