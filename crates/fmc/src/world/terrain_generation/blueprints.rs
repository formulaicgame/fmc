use bevy::prelude::*;
use rand::{distributions::Distribution, Rng};

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::{
    blocks::{BlockId, Blocks, BLOCK_CONFIG_PATH},
    utils,
    world::chunk::Chunk,
};

use super::TerrainFeature;

pub const BLUEPRINT_PATH: &str = "./resources/server/blueprints/";

// Blueprints contain instructions for placing terrain features.
// Many features share the same layout, and even though blueprints are mainly meant to compose
// features, some simple blueprints are made available to ease their creation. If you want to
// create a new type of feature, it is meant to be programmed as a 'Generator'.
#[derive(Clone)]
pub enum Blueprint {
    // A collection of blueprints that will be generated together.
    Collection(Vec<Blueprint>),
    Distribution {
        // The blueprint that should be constructed.
        blueprint: Box<Blueprint>,
        // TODO: This is just a uniform distribution now. Triangle distributions would be nice, but the
        // rand crate implements distributions as a trait which makes them difficult to store since
        // each distribution type has its own struct. Rand is also slow, will probably have to do
        // some homebrew.
        //
        // Number of attempts at placing that should be done for each chunk
        count: u32,
        // If specified it will only distribute between the two height values. If None, it will
        // snap to the surface. [low_y, high_y]
        vertical_range: Option<[i32; 2]>,
    },
    // A function that generates a feature
    Generator(fn(position: IVec3, blocks: &mut TerrainFeature)),
    // TODO: There's room to introduce branches without cluttering the interface too much I think.
    // TODO: Some way to specify canopy style.
    //
    // The generic tree, a straight trunk with a bush on top.
    Tree {
        // Block used as trunk
        trunk_block: BlockId,
        // Block used as leaves
        leaf_block: BlockId,
        // Temp value while all trees share the same canopy. To avoid having to reallocate it every
        // time.
        canopy_clipper: rand::distributions::Bernoulli,
        // Minimum height of the tree
        trunk_height: i32,
        // A random integer between 0 and random_height is added to the trunk height.
        random_height: rand::distributions::Uniform<i32>,
        // How many blocks wide the trunk should be
        trunk_width: u32,
        // Which blocks the tree can grow from.
        soil_blocks: HashSet<BlockId>,
        // Which blocks the tree can replace when it grows.
        can_replace: HashSet<BlockId>,
    },
    // An ore vein
    OreVein {
        /// The block that is placed
        ore_block: BlockId,
        /// The number of ore blocks that are placed.
        count: u32,
        /// Which blocks the ore can be placed into.
        can_replace: HashSet<BlockId>,
    },
}

impl Blueprint {
    fn new(
        json_blueprint: &AmbiguousJsonBlueprint,
        named_blueprints: &HashMap<String, AmbiguousJsonBlueprint>,
        blocks: &Blocks,
    ) -> Self {
        match json_blueprint {
            AmbiguousJsonBlueprint::Named(name) => Blueprint::new(
                named_blueprints.get(name).unwrap(),
                named_blueprints,
                blocks,
            ),
            AmbiguousJsonBlueprint::Inline(json_blueprint) => match json_blueprint {
                JsonBlueprint::Collection { blueprints, .. } => {
                    let mut collection = Vec::with_capacity(blueprints.len());
                    for sub_blueprint in blueprints {
                        let sub_blueprint = Blueprint::new(sub_blueprint, named_blueprints, blocks);
                        collection.push(sub_blueprint);
                    }
                    Blueprint::Collection(collection)
                }
                JsonBlueprint::Distribution {
                    blueprint,
                    count,
                    vertical_range,
                } => {
                    let sub_blueprint = Blueprint::new(blueprint, named_blueprints, blocks);
                    Blueprint::Distribution {
                        blueprint: Box::new(sub_blueprint),
                        count: *count,
                        vertical_range: vertical_range.clone(),
                    }
                }
                JsonBlueprint::Tree {
                    trunk_block,
                    leaf_block,
                    trunk_height,
                    random_height,
                    trunk_width,
                    soil_blocks,
                    can_replace,
                } => Blueprint::Tree {
                    trunk_block: blocks.get_id(&trunk_block),
                    leaf_block: blocks.get_id(&leaf_block),
                    canopy_clipper: rand::distributions::Bernoulli::new(0.5).unwrap(),
                    trunk_height: *trunk_height as i32,
                    random_height: rand::distributions::Uniform::new_inclusive(
                        0,
                        random_height.unwrap_or(0) as i32,
                    ),
                    trunk_width: *trunk_width,
                    soil_blocks: soil_blocks
                        .iter()
                        .map(|block_name| blocks.get_id(block_name))
                        .collect::<HashSet<BlockId>>(),
                    can_replace: can_replace
                        .iter()
                        .map(|block_name| blocks.get_id(block_name))
                        .collect::<HashSet<BlockId>>(),
                },
                JsonBlueprint::OreVein {
                    ore_block,
                    count,
                    can_replace,
                } => Blueprint::OreVein {
                    ore_block: blocks.get_id(&ore_block),
                    count: *count,
                    can_replace: can_replace
                        .iter()
                        .map(|block_name| blocks.get_id(block_name))
                        .collect::<HashSet<BlockId>>(),
                },
            },
        }
    }

    // TODO: The surface parameter is too constricting, a blueprint might want to know all open
    // faces be it floor, roof or wall, below or above ground. Idk how to do it.
    pub fn construct(
        &self,
        chunk_position: IVec3,
        surface: &Vec<Option<(usize, BlockId)>>,
        rng: &mut rand::rngs::StdRng,
    ) -> TerrainFeature {
        let mut feature = TerrainFeature {
            blocks: HashMap::new(),
            can_replace: HashSet::new(),
        };
        self._construct(chunk_position, surface, rng, &mut feature);

        return feature;
    }

    fn _construct(
        &self,
        origin: IVec3,
        surface: &Vec<Option<(usize, BlockId)>>,
        rng: &mut rand::rngs::StdRng,
        feature: &mut TerrainFeature,
    ) {
        match self {
            Blueprint::Collection(blueprints) => {
                for blueprint in blueprints {
                    blueprint._construct(origin, surface, rng, feature);
                }
            }
            Blueprint::Distribution {
                blueprint,
                count,
                vertical_range,
            } => {
                if let Some(vertical_range) = vertical_range {
                    if vertical_range[0] > origin.y || vertical_range[1] < origin.y {
                        return;
                    }
                };

                let distribution = rand::distributions::Uniform::new(0, Chunk::SIZE.pow(3));
                for _ in 0..*count {
                    let position =
                        origin + utils::block_index_to_position(rng.sample(distribution));
                    blueprint._construct(position, surface, rng, feature);
                }
            }
            Blueprint::Generator(generator_function) => {
                generator_function(origin, feature);
            }
            // TODO: Trunk width
            Blueprint::Tree {
                trunk_block,
                leaf_block,
                canopy_clipper,
                trunk_height,
                random_height,
                trunk_width: _trunk_width,
                soil_blocks,
                can_replace,
            } => {
                // The distribution goes over a 3d space, so we convert it to 2d and set the y to
                // whatever the surface height is at that position.
                let (chunk_position, index) =
                    utils::world_position_to_chunk_position_and_block_index(origin);
                let index = index >> 4;
                let (surface_y, surface_block) = match &surface[index] {
                    Some(s) => s,
                    None => return,
                };

                if !soil_blocks.contains(surface_block) {
                    return;
                }

                let mut position = origin;
                position.y = chunk_position.y + *surface_y as i32;

                feature.can_replace.extend(can_replace);

                let height = trunk_height + random_height.sample(rng);
                for height in 1..=height {
                    feature.insert_block(position + IVec3::new(0, height, 0), *trunk_block);
                }

                // Insert two bottom leaf layers.
                for y in height - 2..height {
                    for x in -2..=2 {
                        for z in -2..=2 {
                            if (x == 2 || x == -2)
                                && (z == 2 || z == -2)
                                && canopy_clipper.sample(rng)
                            {
                                // Remove 50% of edges for more variance
                                continue;
                            }
                            feature.insert_block(
                                IVec3 {
                                    x: position.x + x,
                                    y: position.y + y,
                                    z: position.z + z,
                                },
                                *leaf_block,
                            );
                        }
                    }
                }

                // Insert top layer of leaves.
                for y in height..=height + 1 {
                    for x in -1..=1 {
                        for z in -1..=1 {
                            if (x == 1 || x == -1)
                                && (z == 1 || z == -1)
                                && canopy_clipper.sample(rng)
                            {
                                continue;
                            }
                            feature.insert_block(
                                IVec3 {
                                    x: position.x + x,
                                    y: position.y + y,
                                    z: position.z + z,
                                },
                                *leaf_block,
                            );
                        }
                    }
                }
            }
            Blueprint::OreVein {
                ore_block,
                count,
                can_replace,
            } => {
                // TODO: Implement as const when making rand lib
                let directions = rand::distributions::Slice::<IVec3>::new(&[
                    IVec3::X,
                    IVec3::NEG_X,
                    IVec3::Y,
                    IVec3::NEG_Y,
                    IVec3::Z,
                    IVec3::NEG_Z,
                ])
                .unwrap();

                let mut position = origin;
                for direction in directions.sample_iter(rng).take(*count as usize) {
                    position += *direction;
                    feature.insert_block(position, *ore_block)
                }

                feature.can_replace.extend(can_replace);
            }
        }
    }
}

// This allows json blueprints to be nested in an ergonomic way in exchange for less ergonomic
// code.
//
// named:
// {
//     type: some_blueprint_type,
//     field_1: some_value,
//     nested_blueprint: "blueprint_1"
// }
//
// inline:
// {
//     type: some_blueprint_type
//     field_1: some_value,
//     nested_blueprint: {
//         type: some_blueprint_type,
//         field_1: some_value,
//         ...
//     }
// }
#[derive(Deserialize)]
#[serde(untagged)]
enum AmbiguousJsonBlueprint {
    Named(String),
    Inline(JsonBlueprint),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum JsonBlueprint {
    Collection {
        blueprints: Vec<AmbiguousJsonBlueprint>,
    },
    Distribution {
        blueprint: Box<AmbiguousJsonBlueprint>,
        count: u32,
        vertical_range: Option<[i32; 2]>,
    },
    Tree {
        trunk_block: String,
        leaf_block: String,
        trunk_height: u32,
        random_height: Option<u32>,
        trunk_width: u32,
        soil_blocks: Vec<String>,
        can_replace: Vec<String>,
    },
    OreVein {
        ore_block: String,
        count: u32,
        can_replace: Vec<String>,
    },
}

pub fn load_blueprints(blocks: &Blocks) -> HashMap<String, Blueprint> {
    let mut named_json_blueprints = HashMap::new();

    let directory = std::fs::read_dir(BLUEPRINT_PATH).expect(&format!(
        "Could not read files from blueprints directory, make sure it is present as '{}'",
        BLUEPRINT_PATH
    ));

    for entry in directory {
        let file_path = entry
            .expect("Failed to read the filenames of the block configs")
            .path();

        let file = std::fs::File::open(&file_path).expect(&format!(
            "Failed to open blueprint file at '{}'",
            file_path.display()
        ));
        let blueprint = serde_json::from_reader(file).expect(&format!(
            "Failed to read blueprint at '{}'",
            file_path.display()
        ));
        let name = file_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        named_json_blueprints.insert(name, blueprint);
    }

    fn validate_blueprint(
        parent_name: &str,
        child_name: &str,
        named_blueprints: &HashMap<String, AmbiguousJsonBlueprint>,
    ) {
        if !named_blueprints.contains_key(child_name) {
            panic!(
                "Failed while validating the Feature Blueprints. The blueprint '{}', \
                depends on another blueprint '{}', but it could not be found. This is most \
                likely the result of a missing file at '{}', make sure it is present.",
                parent_name,
                child_name,
                BLUEPRINT_PATH.to_owned() + child_name + ".json"
            );
        }
    }

    fn validate_block(blueprint_name: &str, block_name: &str, blocks: &Blocks) {
        if !blocks.contains_block(block_name) {
            panic!(
                "Failed while validating the Feature Blueprints. The blueprint '{}' \
                references a block with the name '{}', but no block by that name exists. \
                Make sure a block by the same name is present at '{}'",
                blueprint_name, block_name, BLOCK_CONFIG_PATH
            );
        }
    }

    for (blueprint_name, json_blueprint) in named_json_blueprints.iter() {
        match json_blueprint {
            AmbiguousJsonBlueprint::Named(child_name) => {
                validate_blueprint(blueprint_name, child_name, &named_json_blueprints)
            }
            AmbiguousJsonBlueprint::Inline(json_blueprint) => match json_blueprint {
                JsonBlueprint::Collection { blueprints } => {
                    for child_blueprint in blueprints {
                        if let AmbiguousJsonBlueprint::Named(child_name) = child_blueprint {
                            validate_blueprint(blueprint_name, child_name, &named_json_blueprints)
                        }
                    }
                }
                JsonBlueprint::Distribution { blueprint, .. } => {
                    if let AmbiguousJsonBlueprint::Named(child_name) = blueprint.as_ref() {
                        validate_blueprint(blueprint_name, child_name, &named_json_blueprints)
                    }
                }
                JsonBlueprint::Tree {
                    trunk_block,
                    leaf_block,
                    soil_blocks,
                    can_replace,
                    ..
                } => {
                    validate_block(blueprint_name, &trunk_block, blocks);
                    validate_block(blueprint_name, &leaf_block, blocks);
                    for block_name in soil_blocks.iter() {
                        validate_block(blueprint_name, block_name, blocks)
                    }
                    for block_name in can_replace.iter() {
                        validate_block(blueprint_name, block_name, blocks)
                    }
                }
                JsonBlueprint::OreVein {
                    ore_block,
                    can_replace,
                    ..
                } => {
                    validate_block(blueprint_name, &ore_block, blocks);
                    for block_name in can_replace.iter() {
                        validate_block(blueprint_name, block_name, blocks)
                    }
                }
            },
        }
    }

    let mut blueprints = HashMap::new();

    for (name, json_blueprint) in named_json_blueprints.iter() {
        let blueprint = Blueprint::new(&json_blueprint, &named_json_blueprints, blocks);
        blueprints.insert(name.to_owned(), blueprint);
    }

    return blueprints;
}
