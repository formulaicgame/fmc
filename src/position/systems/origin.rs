use bevy::prelude::*;
use fmc_world::transform::VoxelTransform;

use crate::{
    position::{components::FloatingOrigin, resources::RenderOrigin},
    world::Origin,
};

pub fn update_origin(
    focus: Single<&VoxelTransform, (With<FloatingOrigin>, Changed<VoxelTransform>)>,
    mut origin: ResMut<RenderOrigin>,
) {
    origin.set_if_neq(RenderOrigin(focus.chunk));
}

// TEMP: delete when old origin is removed
pub fn bridge_origin(old: Res<Origin>, mut new: ResMut<RenderOrigin>) {
    if old.is_changed() {
        new.set_if_neq(RenderOrigin(old.as_chunk_pos()));
    }
}
