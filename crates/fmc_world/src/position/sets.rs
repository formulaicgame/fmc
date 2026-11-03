use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemSet)]
pub enum PositionSystems {
    SyncOrigin,
}
