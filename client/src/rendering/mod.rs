use bevy::prelude::*;

// TODO: This pub is needed for ExpandedChunk, move the struct to the chunk file and close this off.
pub mod chunk;

mod lighting;
pub mod materials;
mod sky;

pub struct RenderingPlugin;
impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(materials::MaterialsPlugin)
            .add_plugins(chunk::ChunkMeshPlugin)
            .add_plugins(lighting::LightingPlugin)
            .add_plugins(sky::SkyPlugin);
        app.configure_sets(
            Update,
            (RenderSet::UpdateBlocks, RenderSet::Light, RenderSet::Mesh).chain(),
        );
    }
}

// The update blocks -> relight -> mesh sequence needs to happen in the same frame for visual
// responsiveness. Mainly necessary for breaking blocks, where the server will remove both the
// model for the "breaking" texture and the block the same tick. Delay would cause you to see the
// block when the breaking texture is gone already, it happens fast, but is visually jarring.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RenderSet {
    UpdateBlocks,
    Light,
    Mesh,
}
