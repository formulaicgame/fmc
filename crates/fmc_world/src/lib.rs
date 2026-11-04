pub mod coords;
pub mod transform;

use bevy::app::plugin_group;

plugin_group! {
    #[derive(Debug)]
    pub struct WorldPlugins {
        transform:::VoxelTransformPlugin,
    }
}
