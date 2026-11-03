use bevy::prelude::*;

use crate::position::components::WorldPos;

#[derive(Component)]
#[require(WorldPos)]
pub struct SyncOrigin;
