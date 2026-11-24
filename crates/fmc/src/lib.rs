/// Asset management
pub mod assets;
pub mod audio;
/// The game's blocks.
pub mod blocks;
pub mod commands;
/// Database connection management
pub mod database;
/// UI interaction infrastructure
pub mod interfaces;
/// The game's items.
pub mod items;
/// The game's models.
pub mod models;
pub mod networking;
pub mod particle_effects;
pub mod permissions;
pub mod physics;
/// Basic player functionality
pub mod players;
pub mod random;
pub mod terminal;
/// f64 transforms
pub mod transform;
pub mod visibility;
/// The block world
pub mod world;

/// SIMD accelerated gradient noise
#[doc(inline)]
pub use fmc_noise as noise;
/// Network protocol
#[doc(inline)]
pub use fmc_protocol as protocol;

pub use bevy_app as app;
pub use bevy_ecs as ecs;
pub use bevy_math as math;
pub use bevy_tasks as tasks;
pub use bevy_time as time;

pub mod prelude {
    // XXX: https://github.com/bevyengine/bevy/issues/9831
    // This means derive(Resource/Component) can only be used when the prelude is imported
    #[doc(hidden)]
    pub use bevy_ecs;

    pub use crate::transform::prelude::*;
    pub use bevy_app::prelude::*;
    pub use bevy_derive::{Deref, DerefMut};
    pub use bevy_ecs::prelude::*;
    pub use bevy_log::prelude::*;
    pub use bevy_math::prelude::*;
    pub use bevy_time::prelude::*;
    pub use bevy_utils::prelude::*;
}

use bevy_app::{PluginGroup, PluginGroupBuilder};

/// Enables all available functionality.
///
/// Click `source` above to see the list of plugins.\
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
    // ApplyDeffered when it's necessary if the sets are chained.
    fn build(self) -> PluginGroupBuilder {
        let group = PluginGroupBuilder::start::<Self>();
        group
            .add(crate::app::ScheduleRunnerPlugin::run_loop(
                // ~60 ticks a second
                std::time::Duration::from_millis(16),
            ))
            .add(bevy_app::TaskPoolPlugin::default())
            .add(bevy_time::TimePlugin::default())
            .add(bevy_log::LogPlugin::default())
            .add(crate::transform::TransformPlugin)
            .add(bevy_app::TerminalCtrlCHandlerPlugin)
            .add(assets::AssetPlugin)
            .add(database::DatabasePlugin)
            .add(permissions::PermissionsPlugin)
            .add(commands::CommandPlugin)
            .add(terminal::TerminalPlugin)
            .add(networking::ServerPlugin)
            .add(world::WorldPlugin)
            .add(blocks::BlockPlugin)
            .add(items::ItemPlugin)
            .add(visibility::VisibilityPlugin)
            .add(models::ModelPlugin)
            .add(particle_effects::ParticleEffectPlugin)
            .add(physics::PhysicsPlugin)
            .add(players::PlayersPlugin)
            .add(interfaces::InterfacePlugin)
    }
}

/// The bevy crates fmc uses internally, laid out like `bevy` so paths stay familiar.
mod bevy {
    pub use bevy_app as app;
    pub use bevy_ecs as ecs;
    pub use bevy_math as math;
    pub use bevy_platform as platform;
    pub use bevy_tasks as tasks;
    pub use bevy_time as time;

    pub mod prelude {
        pub use bevy_app::prelude::*;
        pub use bevy_derive::{Deref, DerefMut};
        pub use bevy_ecs::prelude::*;
        pub use bevy_log::prelude::*;
        pub use bevy_math::prelude::*;
        pub use bevy_time::prelude::*;
        pub use bevy_utils::prelude::*;
    }
}
