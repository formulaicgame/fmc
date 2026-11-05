use bevy::prelude::*;
use fmc_world::transform::VoxelTransform;

use crate::position::resources::RenderOrigin;

pub fn sync_transforms(
    origin: Res<RenderOrigin>,
    mut transforms: Query<(&VoxelTransform, &mut Transform), Changed<VoxelTransform>>,
) {
    if origin.is_changed() {
        return;
    }

    for (voxel, mut transform) in &mut transforms {
        transform.set_if_neq(voxel.to_render(**origin));
    }
}
