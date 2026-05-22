use std::collections::HashMap;

use bevy::prelude::*;

use crate::assets::AssetSet;

pub const PARTICLE_EFFECT_PATH: &str = "./assets/client/particle_effects/";

pub type ParticleEffectId = u32;

pub struct ParticleEffectPlugin;
impl Plugin for ParticleEffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreStartup,
            load_particle_effects.in_set(AssetSet::ParticleEffects),
        );
    }
}

fn load_particle_effects(mut commands: Commands) {
    let directory = std::fs::read_dir(PARTICLE_EFFECT_PATH).expect(&format!(
        "Could not read files from particle effect directory, make sure it is present at '{}'.",
        PARTICLE_EFFECT_PATH
    ));

    let mut effects = ParticleEffects {
        ids: HashMap::new(),
    };

    for (id, dir_entry) in directory.enumerate() {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => panic!(
                "Failed to read the filename of a particle effect, Error: {}",
                e
            ),
        };

        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        effects.ids.insert(name, id as ParticleEffectId);
    }

    commands.insert_resource(effects);
}

/// Maps each particle effect's filename to a unique id.
#[derive(Resource, Default)]
pub struct ParticleEffects {
    ids: HashMap<String, ParticleEffectId>,
}

impl ParticleEffects {
    pub fn get_id(&self, name: &str) -> Option<ParticleEffectId> {
        self.ids.get(name).cloned()
    }

    pub fn ids(&self) -> &HashMap<String, ParticleEffectId> {
        &self.ids
    }
}
