use fmc::prelude::*;

mod crafting_table;
mod water;
mod wheat;

pub(super) struct BlocksPlugin;
impl Plugin for BlocksPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crafting_table::CraftingTablePlugin)
            .add_plugins(wheat::WheatPlugin)
            .add_plugins(water::WaterPlugin);
    }
}
