use std::collections::HashMap;

use fmc::{
    blocks::{BlockId, Blocks, BLOCK_CONFIG_PATH},
    world::blueprints::{load_blueprints, Blueprint, BLUEPRINT_PATH},
};

pub struct Biome {
    pub top_layer_block: BlockId,
    pub mid_layer_block: BlockId,
    pub bottom_layer_block: BlockId,
    pub surface_liquid: BlockId,
    pub sub_surface_liquid: BlockId,
    pub air: BlockId,
    pub sand: BlockId,
    pub blueprints: Vec<Blueprint>,
}

struct BiomeJson {
    top_layer_block: String,
    mid_layer_block: String,
    bottom_layer_block: String,
    surface_liquid: String,
    sub_surface_liquid: String,
    air: String,
    sand: String,
    blueprints: Vec<String>,
}

// TODO: Create dynamically so it's easier to change. Should be able to add biomes between
// intervals and error if they overlap.
pub struct Biomes {
    biomes: [Biome; 1],
}

impl Biomes {
    pub fn load(blocks: &Blocks) -> Self {
        // TODO: Biomes should be loaded from file, and shouldn't look like this. No sand, air =
        // filler. Not finished because I haven't decided on the biome model yet.
        let biome_name = "base".to_owned();
        let base_biome = BiomeJson {
            top_layer_block: "grass".to_owned(),
            mid_layer_block: "dirt".to_owned(),
            bottom_layer_block: "stone".to_owned(),
            surface_liquid: "surface_water".to_owned(),
            sub_surface_liquid: "subsurface_water".to_owned(),
            air: "air".to_owned(),
            sand: "sand".to_owned(),
            blueprints: vec!["distribute_trees".to_owned(), "coal_ore".to_owned()],
        };

        fn validate_block(biome_name: &str, block_name: &str, blocks: &Blocks) {
            if !blocks.contains_block(block_name) {
                panic!(
                    "Startup failed while validating the biomes. The biome '{}' \
                    references a block with the name '{}', but no block by that name exists. \
                    Make sure a block by the same name is present at '{}'",
                    biome_name, block_name, BLOCK_CONFIG_PATH
                );
            }
        }

        fn validate_blueprint(
            biome_name: &str,
            blueprint_name: &str,
            blueprints: &HashMap<String, Blueprint>,
        ) {
            if !blueprints.contains_key(blueprint_name) {
                panic!(
                    "Failed while validating the biomes. The biome '{}' depends on a blueprint by \
                    the name '{}', but no such blueprint file exists. This is most likely the result of \
                    a missing file at '{}', make sure it is present.",
                    biome_name, blueprint_name, BLUEPRINT_PATH
                );
            }
        }

        validate_block(&biome_name, &base_biome.top_layer_block, blocks);
        validate_block(&biome_name, &base_biome.mid_layer_block, blocks);
        validate_block(&biome_name, &base_biome.bottom_layer_block, blocks);
        validate_block(&biome_name, &base_biome.surface_liquid, blocks);
        validate_block(&biome_name, &base_biome.sub_surface_liquid, blocks);
        validate_block(&biome_name, &base_biome.air, blocks);
        validate_block(&biome_name, &base_biome.sand, blocks);

        let blueprints = load_blueprints(blocks);
        for blueprint_name in base_biome.blueprints.iter() {
            validate_blueprint(&biome_name, blueprint_name, &blueprints);
        }

        let base_biome = Biome {
            top_layer_block: blocks.get_id(&base_biome.top_layer_block),
            mid_layer_block: blocks.get_id(&base_biome.mid_layer_block),
            bottom_layer_block: blocks.get_id(&base_biome.bottom_layer_block),
            surface_liquid: blocks.get_id(&base_biome.surface_liquid),
            sub_surface_liquid: blocks.get_id(&base_biome.sub_surface_liquid),
            air: blocks.get_id(&base_biome.air),
            sand: blocks.get_id(&base_biome.sand),
            blueprints: base_biome
                .blueprints
                .iter()
                .map(|name| blueprints[name].clone())
                .collect(),
        };

        return Biomes {
            biomes: [base_biome],
        };
    }

    // TODO: When implementing this, remember that the call sites also cheat.
    pub fn get_biome(&self) -> &Biome {
        return &self.biomes[0];
    }
}
