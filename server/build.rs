// This compresses the assets into an archive so that it can be included in the executable.
fn main() {
    println!("cargo:rerun-if-changed=resources");

    let mut archive = tar::Builder::new(Vec::new());
    archive
        .append_dir_all("resources/client", "resources/client")
        .unwrap();
    //archive
    //    .append_dir_all("resources/server", "resources/server")
    //    .unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let compressed: Vec<u8> =
        zstd::stream::encode_all(archive.into_inner().unwrap().as_slice(), 19).unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("assets.tar.zstd");

    std::fs::write(dest_path, compressed).unwrap();
}
