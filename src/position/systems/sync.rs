use bevy::prelude::*;
use fmc_world::transform::VoxelTransform;

use crate::position::resources::RenderOrigin;

pub fn sync_transforms(
    origin: Res<RenderOrigin>,
    mut transforms: Query<(Ref<VoxelTransform>, &mut Transform)>,
) {
    let origin_moved = origin.is_changed();

    for (voxel, mut transform) in &mut transforms {
        if origin_moved || voxel.is_changed() {
            transform.set_if_neq(voxel.to_render(**origin));
        }
    }
}
