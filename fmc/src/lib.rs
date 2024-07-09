pub mod assets;
pub mod blocks;
pub mod chat;
pub mod database;
pub mod interfaces;
pub mod items;
pub mod models;
pub mod networking;
pub mod physics;
pub mod players;
pub mod utils;
pub mod world;

pub use bevy;
pub use noise;

pub mod prelude {
    // XXX: https://github.com/bevyengine/bevy/issues/9831
    pub use bevy::ecs as bevy_ecs;

    pub use bevy::prelude::*;
    // Shadow bevy's inbuilt transforms
    pub use crate::bevy_extensions::f64_transform::GlobalTransform;
    pub use crate::bevy_extensions::f64_transform::Transform;
}

mod bevy_extensions;
pub mod transform {
    pub use crate::bevy_extensions::f64_transform::GlobalTransform;
    pub use crate::bevy_extensions::f64_transform::Transform;
    pub use crate::bevy_extensions::f64_transform::TransformBundle;
    pub use crate::bevy_extensions::f64_transform::TransformPlugin;
}

use bevy::app::{PluginGroup, PluginGroupBuilder};
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
                std::time::Duration::from_millis(16),
            ))
            .add(bevy::core::TaskPoolPlugin::default())
            .add(bevy::time::TimePlugin::default())
            .add(bevy::hierarchy::HierarchyPlugin::default())
            .add(bevy::log::LogPlugin::default())
            .add(transform::TransformPlugin)
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
            .add(chat::ChatPlugin)
    }
}
