use bevy::prelude::*;

use crate::position::components::ChunkPos;

#[derive(Resource, Deref, DerefMut)]
pub struct RenderOrigin(ChunkPos);
