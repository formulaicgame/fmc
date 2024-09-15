use std::hash::{DefaultHasher, Hasher};

use bevy::prelude::*;

pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, make_asset_tarball);
    }
}

#[derive(Resource)]
pub struct Assets {
    pub hash: u64,
    pub asset_message: Vec<u8>,
}

fn make_asset_tarball(mut commands: Commands) {
    let possibly_changed_assets = build_asset_archive();

    if let Ok(saved_assets) = std::fs::read("resources/assets.tar.zstd") {
        if hash(&saved_assets) != hash(&possibly_changed_assets) {
            // Tarball doesn't match the asset directory (something added since last run)
            std::fs::write("resources/assets.tar.zstd", &possibly_changed_assets).unwrap();
        }
    } else {
        // Assets haven't been saved to a tarball yet
        std::fs::write("resources/assets.tar.zstd", &possibly_changed_assets).unwrap();
    }

    commands.insert_resource(Assets {
        hash: hash(&possibly_changed_assets),
        asset_message: possibly_changed_assets,
    });
}

fn hash(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(data);
    hasher.finish()
}

/// Creates an archive from all the assets in the client assets directory
fn build_asset_archive() -> Vec<u8> {
    let mut archive = tar::Builder::new(Vec::new());
    archive.append_dir_all(".", "resources/client").unwrap();

    let archive = archive.into_inner().unwrap();

    // TODO: This needs to be max compressed, as it will probably reach hundreds of megabytes.
    // Compressing it would take several seconds each startup. Need to store uncrompressed archive
    // hash so that we know whether we need to rebuild, avoid compressing.
    let compressed = zstd::encode_all(&archive[..], 5).unwrap();

    compressed
}
