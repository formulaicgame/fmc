use std::hash::{DefaultHasher, Hasher};

use bevy::prelude::*;
use fmc_protocol::{messages, MessageType};

/// Manages the server's assets
pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, compress_assets);
        app.configure_sets(
            PreStartup,
            AssetSet::Blocks
                .after(AssetSet::Items)
                .after(AssetSet::Models),
        );
    }
}

/// A [SystemSet] to manage the order assets are loaded in.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum AssetSet {
    Models,
    Items,
    // Blocks rely on Models, and Items
    Blocks,
}

/// A [MessageType::AssetResponse] of the client assets, compressed and ready to be sent to the
/// clients when they connect.
#[derive(Resource)]
pub struct Assets {
    pub hash: u64,
    pub asset_message: Vec<u8>,
}

fn compress_assets(mut commands: Commands) {
    let mut archive = tar::Builder::new(Vec::new());
    archive.append_dir_all(".", "assets/client").unwrap();

    let archive = messages::AssetResponse {
        file: archive.into_inner().unwrap(),
    };

    let hash = hash(&archive.file);

    let mut message = vec![0; 5];

    message[0] = MessageType::AssetResponse as u8;
    bincode::serialize_into(&mut message, &archive).unwrap();
    let size = message.len() as u32 - 5;
    message[1..5].copy_from_slice(&size.to_le_bytes());

    // TODO: This needs to be max compressed, as it will probably reach hundreds of megabytes.
    // Compressing it would take several seconds each startup. Need to store uncrompressed archive
    // hash so that we know whether we need to rebuild, avoid compressing.
    let mut compressed = vec![0; 4];
    zstd::stream::copy_encode(message.as_slice(), &mut compressed, 5).unwrap();
    let size = compressed.len() as u32 - 4;
    compressed[0..4].copy_from_slice(&size.to_le_bytes());

    // if let Ok(saved_assets) = std::fs::read("assets/assets.tar.zstd") {
    //     if hash(&saved_assets) != hash(&possibly_changed_assets) {
    //         // Tarball doesn't match the asset directory (something added since last run)
    //         std::fs::write("assets/assets.tar.zstd", &possibly_changed_assets).unwrap();
    //     }
    // } else {
    //     // Assets haven't been saved to a tarball yet
    //     std::fs::write("assets/assets.tar.zstd", &possibly_changed_assets).unwrap();
    // }

    commands.insert_resource(Assets {
        hash,
        asset_message: compressed,
    });
}

fn hash(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(data);
    hasher.finish()
}
