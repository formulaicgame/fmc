use std::{collections::HashMap, io::prelude::*};

use bevy::{
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};

use crate::networking::NetworkClient;

const TEXTURE_PATH: &str = "server_assets/active/textures/blocks";

/// A lookup table for the texture array. Inserted as ressource. Used while loading the block
/// configs.
#[derive(Resource, Debug)]
pub struct BlockTextures {
    pub handle: Handle<Image>,
    // XXX: Even though the id is stored as u32 the texture array only has 19 bits of indices
    // because of bit packing in the shaders.
    texture_array_indices: HashMap<String, u32>,
}

impl BlockTextures {
    pub fn get(&self, name: &str) -> Option<&u32> {
        return self.texture_array_indices.get(name);
    }
}

/// Stiches all the textures used by blocks into a texture array.
pub fn load_block_textures(
    mut commands: Commands,
    net: Res<NetworkClient>,
    mut images: ResMut<Assets<Image>>,
) {
    // size of 16*16 png 8 bit indexed png
    let mut image_buffer = Vec::with_capacity(256);

    let mut texture_array_indices: HashMap<String, u32> = HashMap::new();

    let mut final_image_data: Vec<u8> = Vec::new();
    let mut id = 0;

    let directory = match std::fs::read_dir(TEXTURE_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured assets: Failed to read from the block texture directory at '{}'\n\
                Error: {}",
                TEXTURE_PATH, e
            ));
            return;
        }
    };

    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(format!(
                    "Failed to read directory entry in the block texture directory\nError: {}",
                    e
                ));
                return;
            }
        };

        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(format!(
                    "Failed to open block texture at {}\nError: {}",
                    path.display(),
                    e,
                ));
                return;
            }
        };

        // TODO: handle error
        image_buffer.clear();
        file.read_to_end(&mut image_buffer).ok();

        // TODO: Panic not allowed
        let image = Image::from_buffer(
            &image_buffer,
            ImageType::MimeType("image/png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        )
        .unwrap();

        // TODO: Panic not allowed
        assert!(image.size()[0] == 16);

        let id_increment = image.height() / 16;
        // TODO: Panic not allowed?
        final_image_data.extend(image.data.unwrap());

        let name = path.file_name().unwrap().to_string_lossy();
        texture_array_indices.insert(name.to_string(), id);

        id += id_increment;
    }

    let final_image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: id,
        },
        TextureDimension::D2,
        final_image_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    //image::save_buffer(
    //    "/tmp/foo.png",
    //    final_image.data.as_ref(),
    //    final_image.texture_descriptor.size.width,
    //    final_image.texture_descriptor.size.height
    //        * final_image.texture_descriptor.size.depth_or_array_layers,
    //    image::ColorType::Rgba8,
    //).unwrap();

    let block_textures = BlockTextures {
        handle: images.add(final_image),
        texture_array_indices,
    };

    commands.insert_resource(block_textures);
}
