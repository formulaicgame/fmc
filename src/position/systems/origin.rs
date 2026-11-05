use bevy::prelude::*;
use fmc_world::transform::VoxelTransform;

use crate::position::{components::FloatingOrigin, resources::RenderOrigin};

pub fn update_origin(
    focus: Single<&VoxelTransform, With<FloatingOrigin>>,
    mut origin: ResMut<RenderOrigin>,
) {
    origin.set_if_neq(RenderOrigin(focus.chunk));
}
