use fmc::prelude::*;

pub struct AssetPlugin;
impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        // The server's necessary assets are included at compile time, two birds one stone.
        // 1. The assets are always available without having to fetch them from the web.
        // TODO: Mods ruin this
        // 2. We do not need to have a list of necessary blocks/items/models included in the
        //    source. Although if compiled without the required assets, it will cause unexpected panics.
        // Every time a new world file is initialized the assets are unpacked without overwriting.
        // The server can then read them and store their ids in the database guaranteed that it
        // will not miss any.
        // Subsequent runs of the world can then verify that its required assets are present.
        let assets = include_bytes!(concat!(env!("OUT_DIR"), "/assets.tar.zstd"));
        let uncompressed = zstd::stream::decode_all(assets.as_slice()).unwrap();
        let mut archive = tar::Archive::new(uncompressed.as_slice());

        for entry in archive.entries().unwrap() {
            let mut file = entry.unwrap();
            let path = file.path().unwrap();
            if !path.exists() {
                match file.unpack_in(".") {
                    Err(e) => panic!("Failed to extract default assets.\nError: {e}"),
                    _ => (),
                }
            }
        }
    }
}
