use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoxelTransformSystems {
    Origin,
    Sync,
    PostPropagate,
}
