// TODO: there's a lot of magic numbers here that reference chunk sizing and
// data. Eventually chunk stuff should be moved over to this crate.

use bevy::app::plugin_group;

pub mod position;

plugin_group! {
    #[derive(Debug)]
    pub struct WorldPlugins {
        position::plugin:::PositionPlugin,
    }
}
