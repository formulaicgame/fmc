#![warn(missing_docs)]
// f64 is needed for precision in large worlds. This creates a lot of unfortunate mess, bevy might
// add it in the future, see: https://github.com/bevyengine/bevy/issues/1680

/// The basic components of the transform crate
mod components;
mod systems;

use bevy_app::{ValidateParentHasComponentPlugin, prelude::*};
use bevy_ecs::prelude::*;
pub use components::{GlobalTransform, Transform};
pub use systems::StaticTransformOptimizations;
use systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms};

/// Set enum for the systems relating to transform propagation
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum TransformSystems {
    /// Propagates changes in transform to children's [`GlobalTransform`](crate::components::GlobalTransform)
    Propagate,
}

/// The base plugin for handling [`Transform`] components
#[derive(Default)]
pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ValidateParentHasComponentPlugin::<GlobalTransform>::default())
            .init_resource::<StaticTransformOptimizations>()
            // add transform systems to startup so the first update is "correct"
            .add_systems(
                PostStartup,
                (
                    mark_dirty_trees,
                    propagate_parent_transforms,
                    sync_simple_transforms,
                )
                    .chain()
                    .in_set(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (
                    mark_dirty_trees,
                    propagate_parent_transforms,
                    // TODO: Adjust the internal parallel queries to make this system more efficiently share and fill CPU time.
                    sync_simple_transforms,
                )
                    .chain()
                    .in_set(TransformSystems::Propagate),
            );
    }
}

/// The transform prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
#[doc(hidden)]
pub mod prelude {
    #[doc(hidden)]
    pub use super::components::{GlobalTransform, Transform};
}
