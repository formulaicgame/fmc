use bevy::prelude::*;

mod block_textures;
mod materials;
pub mod models;
mod plugins;

pub use block_textures::BlockTextures;
pub use materials::Materials;

// Assets are downloaded at connection over in 'src/networking.rs'. It matches the asset hash from
// the server config with the clients stored assets, and requests them if it doesn't have them.
// AssetState::Loading is set from there.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetState {
    #[default]
    Inactive,
    Loading,
}

// TODO: This doesn't actually work, if there's any error it will panic.
// TODO: Loading will have to be async to not lag the client, need to show progress in the gui.
// Would also be nice to make all the functions private to their own modules, it makes the global
// namespace filthy.
pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AssetState>()
            .add_plugins(plugins::WasmPlugin)
            .add_plugins(models::ModelPlugin);

        app.add_systems(
            OnEnter(AssetState::Loading),
            (
                block_textures::load_block_textures,
                models::load_models,
                crate::ui::server::key_bindings::load_key_bindings,
                plugins::load_plugins,
                ApplyDeferred,
                materials::load_materials,
                ApplyDeferred,
                crate::world::blocks::load_blocks,
                ApplyDeferred,
                crate::ui::server::items::load_items,
                crate::ui::server::load_interfaces,
                finish_loading,
            )
                .chain(),
        );
    }
}

fn finish_loading(mut asset_state: ResMut<NextState<AssetState>>) {
    info!("Finished loading assets");
    asset_state.set(AssetState::Inactive);
}
