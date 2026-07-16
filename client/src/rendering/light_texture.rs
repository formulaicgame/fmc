use std::sync::OnceLock;

use bevy::{
    asset::{RenderAssetUsages, load_internal_asset, uuid_handle},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    platform::collections::HashMap,
    prelude::*,
    render::{
        Extract, RenderApp,
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
            TextureAspect, TextureDimension, TextureFormat,
        },
        renderer::RenderQueue,
        texture::GpuImage,
    },
};

use crate::{
    rendering::materials::{BlockMaterial, ModelMaterial, ParticleMaterial},
    settings::Settings,
    world::Chunk,
};

const LIGHTING_SHADER: Handle<Shader> = uuid_handle!("60c95d54-a3b7-4ac1-97d8-3e57f74d4e35");
pub const LIGHT_TEXTURE: Handle<Image> = uuid_handle!("bff6f429-1757-4e88-8a17-467b935b9911");

pub struct LightTexturePlugin;
impl Plugin for LightTexturePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            LIGHTING_SHADER,
            "shaders/lighting.wgsl",
            Shader::from_wgsl
        );

        app.init_resource::<LightTextureUploads>().add_systems(
            Update,
            (
                create_light_texture.run_if(resource_changed::<Settings>),
                upload_light_regions.after(create_light_texture),
            ),
        );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(ExtractSchedule, extract_light_texture);
    }
}

#[derive(Resource)]
pub struct LightTexture {
    // The extent of the texture along each axis in chunks.
    pub size: u32,
    texture: OnceLock<Texture>,
}

static SUNLIGHT: [u8; Chunk::SIZE.pow(3) * 2] = {
    let mut data = [0u8; Chunk::SIZE.pow(3) * 2];
    let mut i = 0;
    while i < data.len() {
        data[i] = 255;
        i += 2;
    }
    data
};
static NO_SUNLIGHT: [u8; Chunk::SIZE.pow(3) * 2] = [0; Chunk::SIZE.pow(3) * 2];

pub struct LightUpload {
    // Two bytes per block, sunlight then artificial, scaled from 0..15 to 0..255.
    pub data: [u8; Chunk::SIZE.pow(3) * 2],
    // Uniform chunks are only ever full sunlight or full shadow. Points to one of SUNLIGHT or
    // NO_SUNLIGHT to save on computation.
    uniform: Option<&'static [u8; Chunk::SIZE.pow(3) * 2]>,
}

impl LightUpload {
    pub fn new(data: [u8; Chunk::SIZE.pow(3) * 2]) -> Self {
        Self {
            data,
            uniform: None,
        }
    }

    pub fn uniform_sunlight() -> Self {
        Self {
            data: [0; Chunk::SIZE.pow(3) * 2],
            uniform: Some(&SUNLIGHT),
        }
    }

    pub fn uniform_dark() -> Self {
        Self {
            data: [0; Chunk::SIZE.pow(3) * 2],
            uniform: Some(&NO_SUNLIGHT),
        }
    }

    /// The light data to upload: the static substitute for a uniform chunk, otherwise `data`.
    fn data(&self) -> &[u8] {
        self.uniform.map_or(&self.data, |data| data)
    }
}

/// Chunk regions waiting to be written into the light texture.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct LightTextureUploads(HashMap<IVec3, LightUpload>);

fn create_light_texture(
    mut commands: Commands,
    settings: Res<Settings>,
    light_texture: Option<Res<LightTexture>>,
    mut images: ResMut<Assets<Image>>,
    mut block_materials: ResMut<Assets<BlockMaterial>>,
    mut model_materials: ResMut<Assets<ModelMaterial>>,
    mut particle_materials: ResMut<Assets<ParticleMaterial>>,
) {
    // One chunk of margin so partially visible chunks at the edge don't wrap around and read the
    // light of the opposite side of the render distance.
    let size = settings.render_distance * 2 + 2;

    if let Some(light_texture) = light_texture {
        if light_texture.size == size {
            return;
        }
    }

    let voxels = UVec3::splat(size * Chunk::SIZE as u32);
    // zero-initialized on first use.
    let mut image = Image::new_uninit(
        Extent3d {
            width: voxels.x,
            height: voxels.y,
            depth_or_array_layers: voxels.z,
        },
        TextureDimension::D3,
        // TODO: Can be converted to single channel to halve memory footprint. Had some idea of
        // doing linear interpolation through sampling but that was bad.
        TextureFormat::Rg8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Linear filtering gives us smooth lighting for free, and the repeat address mode wraps the
    // filtering around the edges so the toroidal adressing stays seamless.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });

    images.insert(&LIGHT_TEXTURE, image).unwrap();

    commands.insert_resource(LightTexture {
        size,
        texture: OnceLock::new(),
    });

    for (_, material) in block_materials.iter_mut() {
        material.origin += 0;
    }

    for (_, material) in model_materials.iter_mut() {
        material.extension.origin += 0;
    }

    for (_, material) in particle_materials.iter_mut() {
        material.origin += 0;
    }
}

fn upload_light_regions(
    render_queue: Res<RenderQueue>,
    light_texture: Res<LightTexture>,
    mut uploads: ResMut<LightTextureUploads>,
) {
    if uploads.is_empty() {
        return;
    }

    let Some(texture) = &light_texture.texture.get() else {
        // The gpu texture hasn't been created yet.
        return;
    };

    const SIZE: u32 = Chunk::SIZE as u32;
    // Chunk positions are in blocks, so the wrap has to be too.
    let texture_size = IVec3::splat((light_texture.size * SIZE) as i32);

    for (chunk_position, upload) in uploads.drain() {
        let chunk_origin = chunk_position.rem_euclid(texture_size).as_uvec3();

        render_queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: chunk_origin.x,
                    y: chunk_origin.y,
                    z: chunk_origin.z,
                },
                aspect: TextureAspect::All,
            },
            upload.data(),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 2),
                rows_per_image: Some(SIZE),
            },
            Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: SIZE,
            },
        );
    }
}

fn extract_light_texture(
    light_texture: Extract<Res<LightTexture>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
) {
    if light_texture.texture.get().is_some() {
        return;
    }

    if let Some(gpu_image) = gpu_images.get(LIGHT_TEXTURE.id()) {
        // The render world takes some time to allocate the new image, in the meantime the handle
        // will point to the previous texture. We have to validate that it is the new one before we
        // assign it.
        // I'm not exactly sure where it happens prepare_assets<GpuImage> perhaps.
        let expected = light_texture.size * Chunk::SIZE as u32;
        let texture = &gpu_image.texture;
        if texture.width() != expected {
            return;
        }

        light_texture.texture.set(texture.clone()).unwrap();
    }
}
