use std::collections::HashMap;

use bevy::{color::Color, prelude::*, render::render_resource::Face};
use serde::Deserialize;

use crate::{
    assets::BlockTextures, networking::NetworkClient, rendering::materials::BlockMaterial,
};

/// Stores all the loaded material handles.
/// They can be accessed by the filename the material was loaded from.
#[derive(Resource, Default)]
pub struct Materials {
    inner: HashMap<String, UntypedHandle>,
}

impl Materials {
    pub fn get(&self, name: &str) -> Option<&UntypedHandle> {
        return self.inner.get(name);
    }

    pub fn insert(&mut self, name: String, handle: UntypedHandle) {
        self.inner.insert(name, handle);
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct MaterialConfig {
    // One of "block" or "standard"
    pub r#type: String,
    pub base_color: Srgba,
    pub base_color_texture: Option<String>,
    pub emissive: Srgba,
    pub emissive_texture: Option<String>,
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub metallic_roughness_texture: Option<String>,
    pub reflectance: f32,
    pub normal_map_texture: Option<String>,
    pub occlusion_texture: Option<String>,
    pub double_sided: bool,
    pub unlit: bool,
    pub fog_enabled: bool,
    pub transparency: String,
    pub animation_frames: u32,
}

impl Default for MaterialConfig {
    fn default() -> Self {
        Self {
            // This field will panic if not in the file.
            r#type: "".to_owned(),
            base_color: Srgba::WHITE,
            base_color_texture: None,
            emissive: Srgba::BLACK,
            emissive_texture: None,
            //perceptual_roughness: 0.089,
            perceptual_roughness: 1.,
            metallic: 0.0,
            metallic_roughness_texture: None,
            reflectance: 0.0,
            normal_map_texture: None,
            occlusion_texture: None,
            double_sided: false,
            unlit: false,
            fog_enabled: true,
            transparency: "opaque".to_owned(),
            animation_frames: 1,
        }
    }
}

// TODO: Want the materials to be configurable at runtime by the server. That way it can do stuff
// like changing how the sun looks or something.
pub fn load_materials(
    net: Res<NetworkClient>,
    mut commands: Commands,
    block_textures: Res<BlockTextures>,
    asset_server: Res<AssetServer>,
    mut block_materials: ResMut<Assets<BlockMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut materials = Materials::default();

    let dir = std::path::PathBuf::from("server_assets/active/materials");
    for dir_entry in std::fs::read_dir(&dir).unwrap() {
        let file_path = match dir_entry {
            Ok(p) => p.path(),
            Err(e) => {
                net.disconnect(format!(
                    "Encountered error reading file entries in directory: {}\n Error: {}",
                    dir.to_string_lossy(),
                    e
                ));
                return;
            }
        };

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(format!(
                    "Failed to open material config.\nPath: {}\nError: {}",
                    file_path.to_string_lossy(),
                    e
                ));
                return;
            }
        };

        let config: MaterialConfig = match serde_json::from_reader(file) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(format!(
                    "Failed to read material configuration, path: {} Error: {}",
                    file_path.to_string_lossy(),
                    e
                ));
                return;
            }
        };

        let base_color_texture: Option<Handle<Image>> = match config.base_color_texture {
            Some(path) => Some(asset_server.load(&path)),
            None => None,
        };

        let emissive_texture: Option<Handle<Image>> = match config.emissive_texture {
            Some(path) => Some(asset_server.load(&path)),
            None => None,
        };

        let metallic_roughness_texture: Option<Handle<Image>> =
            match config.metallic_roughness_texture {
                Some(path) => Some(asset_server.load(&path)),
                None => None,
            };

        let normal_map_texture: Option<Handle<Image>> = match config.normal_map_texture {
            Some(path) => Some(asset_server.load(&path)),
            None => None,
        };

        let occlusion_texture: Option<Handle<Image>> = match config.occlusion_texture {
            Some(path) => Some(asset_server.load(&path)),
            None => None,
        };

        // TODO: Create separate enum that is serializable
        let alpha_mode = match config.transparency.as_str() {
            "opaque" => AlphaMode::Opaque,
            "blend" => AlphaMode::Blend,
            "mask" => AlphaMode::Mask(0.5),
            "multiply" => AlphaMode::Multiply,
            "premultiplied" => AlphaMode::Premultiplied,
            "add" => AlphaMode::Add,
            wrong_one => {
                net.disconnect(format!(
                    "Failed to read material configuration, path: {}\nError: 'transparency' needs to be one of 'opaque', 'mask', 'multiply', 'premultiplied' and 'add'. '{}' is not recognized.",
                    file_path.to_string_lossy(),
                    wrong_one
                ));
                return;
            }
        };

        let handle = if config.r#type == "block" {
            let material = BlockMaterial {
                base_color: config.base_color.into(),
                cull_mode: if config.double_sided {
                    None
                } else {
                    Some(Face::Back)
                },
                alpha_mode,
                depth_bias: 0.0,
                texture_array: Some(block_textures.handle.clone()),
                animation_frames: config.animation_frames,
            };
            block_materials.add(material).untyped()
        } else if config.r#type == "standard" {
            // TODO: Maybe this can be removed, nothing uses it. I can't quite remember what the
            // plan was. Think I thought mobs were to use it.
            let material = StandardMaterial {
                base_color: config.base_color.into(),
                base_color_texture,
                emissive: config.emissive.into(),
                emissive_texture,
                perceptual_roughness: config.perceptual_roughness,
                metallic: config.metallic,
                metallic_roughness_texture,
                reflectance: config.reflectance,
                normal_map_texture,
                occlusion_texture,
                double_sided: config.double_sided,
                unlit: config.unlit,
                fog_enabled: config.fog_enabled,
                alpha_mode,
                ..default()
            };
            standard_materials.add(material).untyped()
        } else {
            net.disconnect(format!(
                "Misconfigured material, path: {}\n 'type' field is wrong, should be one
                    of 'block' or 'standard'",
                &file_path.to_string_lossy().into_owned()
            ));
            return;
        };

        let name = file_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        materials.insert(name, handle);
    }

    commands.insert_resource(materials);
}
