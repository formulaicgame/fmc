use bevy::prelude::*;
use fmc_world::transform::VoxelTransformSystems;

use crate::{
    game_state::GameState,
    position::{resources::RenderOrigin, systems},
};

pub struct PositionPlugin;

impl Plugin for PositionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderOrigin>().add_systems(
            PostUpdate,
            (
                (systems::update_origin, systems::bridge_origin)
                    .in_set(VoxelTransformSystems::Origin),
                systems::sync_transforms.in_set(VoxelTransformSystems::Sync),
                systems::update_global_voxel.in_set(VoxelTransformSystems::PostPropagate),
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}
