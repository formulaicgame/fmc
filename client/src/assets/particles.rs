use std::{collections::HashMap, io::prelude::*};

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};

use fmc_protocol::messages;

use crate::{game_state::GameState, networking::NetworkClient, utils};

const PARTICLE_EFFECT_PATH: &str = "server_assets/active/particle_effects/";

// Particle effect asset ids are provided by the server on connection
pub type ParticleEffectAssetId = u32;

pub(super) struct ParticleEffectPlugin;
impl Plugin for ParticleEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Launcher), cleanup);
    }
}

fn cleanup(mut commands: Commands) {
    commands.remove_resource::<ParticleEffects>();
    commands.remove_resource::<ParticleTextures>();
}

#[derive(Resource, Deref)]
pub struct ParticleTextures(HashMap<String, Handle<Image>>);

impl ParticleTextures {
    pub fn get(&self, name: &str) -> Option<&Handle<Image>> {
        self.0.get(name)
    }
}

#[derive(Resource)]
pub struct ParticleEffects {
    id2effect: HashMap<ParticleEffectAssetId, ParticleEffect>,
    name2id: HashMap<String, ParticleEffectAssetId>,
}

impl ParticleEffects {
    pub fn get(&self, id: &ParticleEffectAssetId) -> Option<&ParticleEffect> {
        self.id2effect.get(id)
    }

    pub fn get_id_by_name(&self, name: &str) -> Option<ParticleEffectAssetId> {
        self.name2id.get(name).cloned()
    }
}

#[derive(serde::Deserialize)]
pub struct ParticleEffect {
    position: Position,
    velocity: Velocity,
    pub count: u32,
    pub lifetime: [f32; 2],
    pub acceleration: Vec3,
    pub friction: Vec3,
    pub collision: bool,
    pub size_range: [f32; 2],
    /// For each particle spawned, render it with a smaller section of the texture. Measured in
    /// 1/16 units, first element is minimum amount, last is max
    pub random_uv: Option<UVec2>,
}

impl ParticleEffect {
    pub fn position(&self, rng: &mut utils::Rng) -> Vec3 {
        match self.position {
            Position::Circle {
                center,
                radius,
                sampling,
            } => {
                let angle = rng.next_f32() * std::f32::consts::TAU;
                let r = match sampling {
                    Sampling::Surface => radius,
                    Sampling::Volume => radius * rng.next_f32().sqrt(),
                };
                center + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
            }
            Position::Sphere {
                center,
                radius,
                sampling,
            } => {
                // https://stackoverflow.com/questions/54544971/how-to-generate-uniform-random-points-inside-d-dimension-ball-sphere
                let r = match sampling {
                    Sampling::Surface => radius,
                    Sampling::Volume => radius * rng.next_f32().cbrt(),
                };
                // Spawn randomly along the sphere surface using Archimedes's theorem
                let theta = rng.next_f32() * std::f32::consts::TAU;
                let z = rng.next_f32() * 2.0 - 1.0;
                let phi = z.acos();
                let sin_phi = phi.sin();
                let direction = Vec3::new(sin_phi * theta.cos(), sin_phi * theta.sin(), z);
                center + direction * r
            }
        }
    }

    pub fn velocity(&self, rng: &mut utils::Rng, position: Vec3) -> Vec3 {
        let (direction, speed) = match self.velocity {
            Velocity::Circle {
                center,
                axis,
                speed,
            } => {
                let axis = axis.normalize();
                let offset = position - center;
                let direction = (offset - axis * offset.dot(axis)).normalize();
                (direction, speed)
            }
            Velocity::Sphere { center, speed } => ((position - center).normalize(), speed),
        };
        direction * (speed[0] + (speed[1] - speed[0]) * rng.next_f32())
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
enum Position {
    Circle {
        center: Vec3,
        radius: f32,
        sampling: Sampling,
    },
    Sphere {
        center: Vec3,
        radius: f32,
        sampling: Sampling,
    },
}

#[derive(Copy, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Sampling {
    Surface,
    Volume,
}

#[derive(serde::Deserialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
enum Velocity {
    Circle {
        center: Vec3,
        axis: Vec3,
        speed: [f32; 2],
    },
    Sphere {
        center: Vec3,
        speed: [f32; 2],
    },
}

pub(super) fn load_particle_effects(
    mut commands: Commands,
    net: Res<NetworkClient>,
    server_config: Res<messages::ServerConfig>,
    mut images: ResMut<Assets<Image>>,
) {
    // TODO: https://github.com/bevyengine/bevy/pull/21628 will be available in 0.18 making all this
    // redundant.
    //
    // size of 16*16 png 8 bit indexed png
    let mut image_buffer = Vec::with_capacity(256);

    let path = "server_assets/active/textures/particles";
    let particle_directory = match std::fs::read_dir(path) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured assets: Failed to read from the particle texture directory at '{}'\n\
                Error: {}",
                path, e
            ));
            return;
        }
    };

    let path = "server_assets/active/textures/blocks";
    let block_directory = match std::fs::read_dir(path) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(format!(
                "Misconfigured assets: Failed to read from the block texture directory at '{}'\n\
                Error: {}",
                path, e
            ));
            return;
        }
    };

    let mut textures = HashMap::new();

    for dir_entry in particle_directory.chain(block_directory) {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(format!("Failed to read directory entry\nError: {}", e));
                return;
            }
        };

        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(format!(
                    "Failed to open texture at {}\nError: {}",
                    path.display(),
                    e,
                ));
                return;
            }
        };

        image_buffer.clear();
        file.read_to_end(&mut image_buffer).ok();

        let mut image = Image::from_buffer(
            &image_buffer,
            ImageType::MimeType("image/png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        )
        .unwrap();

        let rows = image.height() / image.width();
        image.reinterpret_stacked_2d_as_array(rows);

        // NOTE: Bevy automatically infers this based on the dimensions, so if they image is square
        // it will infer TextureViewDimension::D2, which crashes because the shader expects an
        // array.
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..default()
        });

        let name = path.file_name().unwrap().to_string_lossy();
        let parent = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        textures.insert(parent.to_owned() + "/" + &name, images.add(image));
    }

    let directory = match std::fs::read_dir(PARTICLE_EFFECT_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(&format!(
                "Misconfigured assets: Failed to read from the particle effect directory at '{}'\n Error: {}",
                PARTICLE_EFFECT_PATH, e
            ));
            return;
        }
    };

    let mut effects = ParticleEffects {
        id2effect: HashMap::new(),
        name2id: HashMap::new(),
    };

    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: Failed to read the file path of a particle effect\n\
                    Error: {}",
                    e
                ));
                return;
            }
        };

        let name = path.file_stem().unwrap().to_string_lossy().into_owned();

        let Some(id) = server_config.particle_effect_ids.get(&name) else {
            net.disconnect(format!(
                "Misconfigured assets: There's a particle effect named '{}' in the assets, but the server didn't send an id for it.",
                &name
            ));
            return;
        };

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                net.disconnect(&format!(
                    "Failed to open file at '{}'\nError: {e}",
                    path.display()
                ));
                return;
            }
        };

        let effect: ParticleEffect = match serde_json::from_reader(file) {
            Ok(e) => e,
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: Could not parse particle effect at '{}'\nError: {e}",
                    path.display()
                ));
                return;
            }
        };

        effects.name2id.insert(name, *id);
        effects.id2effect.insert(*id, effect);
    }

    for (name, id) in server_config.particle_effect_ids.iter() {
        if !effects.id2effect.contains_key(id) {
            net.disconnect(&format!(
                "Misconfigured assets: Missing particle effect, no particle effect with the name '{}', make sure it is included in the assets",
                name
            ));
        }
    }

    commands.insert_resource(effects);
    commands.insert_resource(ParticleTextures(textures));
}
