use bevy::prelude::*;

use crate::transform::sets::VoxelTransformSystems;

#[derive(Default)]
pub struct VoxelTransformPlugin;

impl Plugin for VoxelTransformPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                VoxelTransformSystems::Origin,
                VoxelTransformSystems::Sync,
                VoxelTransformSystems::PostPropagate,
            )
                .chain(),
        )
        .configure_sets(
            PostUpdate,
            (
                VoxelTransformSystems::Sync.before(TransformSystems::Propagate),
                VoxelTransformSystems::PostPropagate.after(TransformSystems::Propagate),
            ),
        );
    }
}
