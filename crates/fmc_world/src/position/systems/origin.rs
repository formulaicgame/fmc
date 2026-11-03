use bevy::prelude::*;

use crate::position::{
    components::{SyncOrigin, WorldPos},
    resources::RenderOrigin,
};

pub fn update_origin(
    mut origin: ResMut<RenderOrigin>,
    synchronizer: Single<&WorldPos, (With<SyncOrigin>, Changed<WorldPos>)>,
) {
    **origin = synchronizer.chunk;
}

pub fn sync_transforms(origin: Res<RenderOrigin>, mut q: Query<(&WorldPos, &mut Transform)>) {
    let origin_changed = origin.is_changed();

    for (world_pos, mut transform) in &mut q {
        if origin_changed || transform.is_changed() {
            transform.translation = world_pos.relative_to(**origin);
        }
    }
}
