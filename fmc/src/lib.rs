/// Asset management
pub mod assets;
/// The game's blocks.
pub mod blocks;
/// Database connection management
pub mod database;
/// UI interaction infrastructure
pub mod interfaces;
/// The game's items.
pub mod items;
/// The game's models.
pub mod models;
pub mod networking;
pub mod physics;
/// Basic player functionality
pub mod players;
// TODO: This is just rng now, rename
pub mod utils;
/// The block world
pub mod world;

/// SIMD accelerated gradient noise
#[doc(inline)]
pub use fmc_noise as noise;
/// Network protocol
#[doc(inline)]
pub use fmc_protocol as protocol;

mod bevy_extensions;

pub mod bevy {
    // This is huge. Don't want to put strain on Docs.rs by storing 250mb every time I bump
    // the version.
    #[doc(no_inline)]
    pub use bevy::*;

    // We want f64 transforms so we shadow bevy's transforms
    pub mod transform {
        pub use crate::bevy_extensions::f64_transform::GlobalTransform;
        pub use crate::bevy_extensions::f64_transform::Transform;
        pub use crate::bevy_extensions::f64_transform::TransformBundle;
        pub use crate::bevy_extensions::f64_transform::TransformPlugin;
        pub use crate::bevy_extensions::f64_transform::TransformSystem;
    }

    pub mod prelude {
        pub use crate::bevy_extensions::f64_transform::GlobalTransform;
        pub use crate::bevy_extensions::f64_transform::Transform;
        pub use crate::bevy_extensions::f64_transform::TransformBundle;
        pub use crate::bevy_extensions::f64_transform::TransformPlugin;
        pub use crate::bevy_extensions::f64_transform::TransformSystem;
        // TODO: For some reason when you click this in the docs it doesn't bring you to bevy's
        // prelude, but the prelude of bevy_internal
        #[doc(no_inline)]
        pub use bevy::prelude::*;
    }
}

pub mod prelude {
    // XXX: https://github.com/bevyengine/bevy/issues/9831
    #[doc(hidden)]
    pub use bevy::ecs as bevy_ecs;

    #[doc(no_inline)]
    pub use bevy::prelude::*;

    pub use crate::bevy_extensions::f64_transform::GlobalTransform;
    pub use crate::bevy_extensions::f64_transform::Transform;
}

use bevy::app::{PluginGroup, PluginGroupBuilder};

/// Enables basic bevy plugins and all available functionality.
///
/// Click `source` above to see the list of plugins.  
/// To disable plugins:
/// ```rust
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins
///             .build()
///             .disable::<fmc::items::ItemPlugin>()
///         );
/// }
/// ```
pub struct DefaultPlugins;
impl PluginGroup for DefaultPlugins {
    // TODO: It might make sense to increase the amount of cpu threads used by the async compute pool
    // since most of the work done is to produce chunks.
    //
    // TODO: Some resources are inserted at app build, and the rest in the startup schedules. What
    // depends on what is completely opaque. It would be nice to have it be explicit, but I don't
    // want to dirty the namespaces with loading functions to congregate them all in the same spot.
    // Maybe it's possible with systemsets, but I don't know how to flush commands with them.
    // Ideally I would want to just cram everything into Startup and mark each loading function
    // with a .run_if(this_or_that_resource.exists()) and have them magically ordered by bevy.
    // Development: I think this is possible to do with systemsets now. Looks like it does
    // apply_deferred when it's necessary if the sets are chained.
    fn build(self) -> PluginGroupBuilder {
        let group = PluginGroupBuilder::start::<Self>();
        group
            .add(bevy::app::ScheduleRunnerPlugin::run_loop(
                // ~60 ticks a second
                std::time::Duration::from_millis(16),
            ))
            .add(bevy::core::TaskPoolPlugin::default())
            .add(bevy::time::TimePlugin::default())
            .add(bevy::hierarchy::HierarchyPlugin::default())
            .add(bevy::log::LogPlugin::default())
            .add(bevy::transform::TransformPlugin)
            .add(assets::AssetPlugin)
            .add(database::DatabasePlugin::default())
            .add(networking::ServerPlugin)
            .add(world::WorldPlugin)
            .add(blocks::BlockPlugin)
            .add(items::ItemPlugin)
            .add(models::ModelPlugin)
            .add(physics::PhysicsPlugin)
            .add(players::PlayersPlugin)
            .add(interfaces::InterfacePlugin)
    }
}
