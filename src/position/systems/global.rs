use bevy::prelude::*;
use fmc_world::transform::{GlobalVoxelTransform, VoxelTransform};

use crate::position::resources::RenderOrigin;

pub fn update_global_voxel(
    origin: Res<RenderOrigin>,
    mut transforms: Query<(&GlobalTransform, &mut GlobalVoxelTransform), Changed<GlobalTransform>>,
) {
    for (global, mut voxel) in &mut transforms {
        let (scale, rotation, translation) = global.to_scale_rotation_translation();

        let mut v = VoxelTransform::from_chunk_local(**origin, translation);
        v.rotation = rotation;
        v.scale = scale;

        voxel.set_if_neq(GlobalVoxelTransform {
            chunk: v.chunk,
            local: v.local,
            rotation,
            scale,
        });
    }
}
