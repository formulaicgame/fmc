use fmc::prelude::*;

mod duck;
mod pathfinding;

pub struct MobsPlugin;
impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(duck::DuckPlugin);
    }
}
