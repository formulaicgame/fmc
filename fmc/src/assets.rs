use bevy::prelude::*;
use sha1::Digest;

pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, make_asset_tarball);
    }
}

/// Sha1 hash of the asset archive
/// Stored as resource to hand to clients for verification
#[derive(Resource)]
pub struct AssetArchiveHash {
    pub hash: Vec<u8>,
}

fn make_asset_tarball(mut commands: Commands) {
    // If the assets have been changed, this will update tarball that is sent to the clients to reflect
    // the change.
    let possibly_changed_assets = build_asset_archive();

    if let Ok(saved_assets) = std::fs::read("resources/assets.tar") {
        // TODO: Should be able to add new assets to old worlds so you can update server and still
        // play on same world.
        if !is_same_sha1(&saved_assets, &possibly_changed_assets) {
            // Tarball doesn't match the asset directory (something added since last run)
            std::fs::write("resources/assets.tar", &possibly_changed_assets).unwrap();
        }
    } else {
        // Assets haven't been saved to a tarball yet
        std::fs::write("resources/assets.tar", &possibly_changed_assets).unwrap();
    }

    commands.insert_resource(AssetArchiveHash {
        hash: sha1::Sha1::digest(&possibly_changed_assets).to_vec(),
    });
}

/// Check that none of the assets have changed since the last run.
fn is_same_sha1(archive_1: &Vec<u8>, archive_2: &Vec<u8>) -> bool {
    let hash_1 = sha1::Sha1::digest(&archive_1);
    let hash_2 = sha1::Sha1::digest(&archive_2);
    return hash_1 == hash_2;
}

/// Creates an archive from all the assets in the Assets directory
fn build_asset_archive() -> Vec<u8> {
    let mut archive = tar::Builder::new(Vec::new());
    archive.append_dir_all(".", "resources/client").unwrap();
    return archive.into_inner().unwrap();
}
