use bevy::prelude::*;

use crate::position::PositionSystems;

#[cfg(feature = "client")]
use super::systems;

#[derive(Default)]
pub struct PositionPlugin;

impl Plugin for PositionPlugin {
    fn build(&self, app: &mut App) {
        let app = app.configure_sets(
            PostUpdate,
            PositionSystems::SyncOrigin.before(TransformSystems::Propagate),
        );

        #[cfg(not(feature = "client"))]
        {
            // Temporary discard until this plugin also does things server-side.
            _ = app;
        }

        #[cfg(feature = "client")]
        app.add_systems(
            PostUpdate,
            (
                systems::origin::update_origin,
                systems::origin::sync_transforms,
            )
                .chain()
                .in_set(PositionSystems::SyncOrigin),
        );
    }
}
