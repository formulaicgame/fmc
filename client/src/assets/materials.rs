use std::collections::HashMap;

use bevy::{color::Color, prelude::*, render::render_resource::Face};
use serde::Deserialize;

use crate::{
    assets::BlockTextures,
    networking::NetworkClient,
    rendering::materials::{BlockMaterial, ModelMaterial, ModelMaterialExtension},
};

const MODEL_MATERIAL_PATH: &str = "server_assets/active/materials/model";
const BLOCK_MATERIAL_PATH: &str = "server_assets/active/materials/block";

#[derive(Resource)]
pub struct Materials<T: Material + Asset> {
    inner: HashMap<String, Handle<T>>,
}

impl<T: Material + Asset> Materials<T> {
    pub fn get(&self, name: &str) -> Option<&Handle<T>> {
        return self.inner.get(name);
    }

    fn insert(&mut self, name: String, handle: Handle<T>) {
        self.inner.insert(name, handle);
    }
}

impl<T: Material + Asset> Default for Materials<T> {
    fn default() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ModelMaterialConfig {
    // One of "block" or "standard"
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
    pub transparency: Transparency,
}

impl Default for ModelMaterialConfig {
    fn default() -> Self {
        Self {
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
            transparency: Transparency::Opaque,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct BlockMaterialConfig {
    // One of "block" or "standard"
    pub base_color: Srgba,
    pub double_sided: bool,
    pub transparency: Transparency,
    pub animation_frames: u32,
}

impl Default for BlockMaterialConfig {
    fn default() -> Self {
        Self {
            base_color: Srgba::WHITE,
            double_sided: false,
            transparency: Transparency::Opaque,
            animation_frames: 1,
        }
    }
}

// Subset of AlphaMode
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Transparency {
    Opaque,
    Blend,
    Mask,
    Multiply,
    Premultiplied,
    Add,
}

impl Transparency {
    fn as_alpha_mode(&self) -> AlphaMode {
        match self {
            Self::Opaque => AlphaMode::Opaque,
            Self::Blend => AlphaMode::Blend,
            Self::Mask => AlphaMode::Mask(0.5),
            Self::Multiply => AlphaMode::Multiply,
            Self::Premultiplied => AlphaMode::Premultiplied,
            Self::Add => AlphaMode::Add,
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
    mut block_material_assets: ResMut<Assets<BlockMaterial>>,
    mut model_material_assets: ResMut<Assets<ModelMaterial>>,
) {
    match load_model_materials(&mut model_material_assets, &asset_server, &block_textures) {
        Ok(model_materials) => commands.insert_resource(model_materials),
        Err(e) => {
            net.disconnect(&e);
            return;
        }
    }

    match load_block_materials(&mut block_material_assets, &asset_server, &block_textures) {
        Ok(block_materials) => commands.insert_resource(block_materials),
        Err(e) => {
            net.disconnect(&e);
            return;
        }
    }
}

fn load_model_materials(
    model_material_assets: &mut Assets<ModelMaterial>,
    asset_server: &AssetServer,
    block_textures: &BlockTextures,
) -> Result<Materials<ModelMaterial>, String> {
    let mut materials = Materials::default();

    let directory = match std::fs::read_dir(MODEL_MATERIAL_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            return Err(format!(
                "Misconfigured assets: Failed to read from the model materials directory at '{}'\n\
                Error: {}",
                MODEL_MATERIAL_PATH, e
            ));
        }
    };

    for dir_entry in directory {
        let file_path = match dir_entry {
            Ok(p) => p.path(),
            Err(e) => {
                return Err(format!(
                    "Encountered error reading file entries in directory: {}\n Error: {}",
                    MODEL_MATERIAL_PATH, e
                ));
            }
        };

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                return Err(format!(
                    "Failed to open material configuration.\nPath: {}\nError: {}",
                    file_path.display(),
                    e
                ));
            }
        };

        let config: ModelMaterialConfig = match serde_json::from_reader(file) {
            Ok(c) => c,
            Err(e) => {
                return Err(format!(
                    "Failed to read material configuration, path: {} Error: {}",
                    file_path.display(),
                    e
                ));
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

        let standard_material = StandardMaterial {
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
            alpha_mode: config.transparency.as_alpha_mode(),
            ..default()
        };

        let handle = model_material_assets.add(ModelMaterial {
            base: standard_material,
            extension: ModelMaterialExtension {
                block_textures: block_textures.handle.clone(),
            },
        });

        let name = file_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        materials.insert(name, handle);
    }

    return Ok(materials);
}

fn load_block_materials(
    block_material_assets: &mut Assets<BlockMaterial>,
    asset_server: &AssetServer,
    block_textures: &BlockTextures,
) -> Result<Materials<BlockMaterial>, String> {
    let mut materials = Materials::default();

    let directory = match std::fs::read_dir(BLOCK_MATERIAL_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            return Err(format!(
                "Misconfigured assets: Failed to read from block materials directory at '{}'\n\
                Error: {}",
                BLOCK_MATERIAL_PATH, e
            ));
        }
    };

    for dir_entry in directory {
        let file_path = match dir_entry {
            Ok(p) => p.path(),
            Err(e) => {
                return Err(format!(
                    "Encountered error reading file entries in directory: {}\n Error: {}",
                    BLOCK_MATERIAL_PATH, e
                ));
            }
        };

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                return Err(format!(
                    "Failed to open material configuration.\nPath: {}\nError: {}",
                    file_path.display(),
                    e
                ));
            }
        };

        let config: BlockMaterialConfig = match serde_json::from_reader(file) {
            Ok(c) => c,
            Err(e) => {
                return Err(format!(
                    "Failed to read material configuration, path: {} Error: {}",
                    file_path.display(),
                    e
                ));
            }
        };

        let block_material = BlockMaterial {
            base_color: config.base_color.into(),
            cull_mode: if config.double_sided {
                None
            } else {
                Some(Face::Back)
            },
            alpha_mode: config.transparency.as_alpha_mode(),
            depth_bias: 0.0,
            texture_array: Some(block_textures.handle.clone()),
            animation_frames: config.animation_frames,
        };
        let handle = block_material_assets.add(block_material);

        let name = file_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        materials.insert(name, handle);
    }

    return Ok(materials);
}
