// TODO: It should store block configs in the worlds database so that worlds are more portable.
//       Addendum: It should store the entire resource folder.
//       It should instead emit warnings when configs(and other things it was initialized with) go
//       missing, and update the database if a config has been changed.
use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    path::Path,
};

use bevy::{ecs::system::EntityCommands, math::DVec3, prelude::*};
use rand::{distributions::WeightedIndex, prelude::Distribution};
use serde::Deserialize;

use crate::{database::Database, items::ItemId};

pub type BlockId = u16;

pub const BLOCK_CONFIG_PATH: &str = "./resources/client/blocks/";
const BLOCK_MATERIAL_PATH: &str = "./resources/client/materials/";

// TODO: Regretting this, just make it a resource with an Arc inside so it can be cloned for
// terrain generation.
//
// For convenience Blocks are made available as a static. It takes some extra effort to setup.
// The static is not available til after startup. It can be accessed through a resource
// though. The blocks exist there in an unfinished state, waiting to be modified with
// functionality, but can safely be used to extract ids and data from.
static BLOCKS: once_cell::sync::OnceCell<Blocks> = once_cell::sync::OnceCell::new();

pub struct BlockPlugin;
impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_blocks_to_resource)
            .add_systems(PostStartup, move_blocks_resource_to_static);
    }
}

fn load_blocks_to_resource(mut commands: Commands, database: Res<Database>) {
    fn walk_dir<P: AsRef<std::path::Path>>(dir: P) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();

        let directory = std::fs::read_dir(dir).expect(
            "Could not read files from block configuration directory, make sure it is present",
        );

        for entry in directory {
            let file_path = entry
                .expect("Failed to read a path while loading the block configs")
                .path();

            if file_path.is_dir() {
                let sub_files = walk_dir(&file_path);
                files.extend(sub_files);
            } else {
                files.push(file_path);
            }
        }

        files
    }

    let mut blocks = Blocks {
        blocks: Vec::new(),
        ids: database.load_block_ids(),
    };

    let item_ids = database.load_item_ids();

    let mut block_ids = blocks.asset_ids();
    let mut maybe_blocks = Vec::new();
    maybe_blocks.resize_with(block_ids.len(), Option::default);

    let block_materials = load_block_materials();

    for file_path in walk_dir(&crate::blocks::BLOCK_CONFIG_PATH) {
        let block_config_json = match BlockConfigJson::from_file(&file_path) {
            Some(b) => b,
            None => continue,
        };

        let drop = match block_config_json.drop {
            Some(drop) => {
                let kind = match BlockDropKind::from_json(&drop.drop, &item_ids) {
                    Ok(d) => d,
                    Err(e) => {
                        panic!(
                            "Failed to read 'drop' field for block at: {}\nError: {}",
                            file_path.display(),
                            e
                        )
                    }
                };
                Some(BlockDrop {
                    requires_tool: drop.requires_tool,
                    drop: kind,
                })
            }
            None => None,
        };

        // Blocks that are not defined by gltf models are required to use a material. If the
        // material is not opaque, then it is assumed transparent. If it is a model block it is
        // always assumed that it is transparent.
        let is_transparent = if let Some(material_name) = &block_config_json.material {
            match block_materials.get(material_name) {
                Some(m) => m.transparency != "opaque",
                None => panic!(
                    "Failed to find material for block: '{}', no material by the name: '{}'\
                    Make sure the material is present at '{}'.",
                    block_config_json.name, material_name, BLOCK_MATERIAL_PATH
                ),
            }
        } else {
            true
        };

        if let Some(block_id) = block_ids.remove(&block_config_json.name) {
            let block_config = BlockConfig {
                name: block_config_json.name,
                friction: block_config_json.friction,
                hardness: block_config_json.hardness,
                tools: block_config_json.tools,
                drop,
                is_rotatable: block_config_json.is_rotatable,
                is_transparent,
            };

            maybe_blocks[block_id as usize] = Some(Block::new(block_config));
        }
    }

    if block_ids.len() > 0 {
        panic!(
            "Misconfigured resource pack, missing blocks: {:?}",
            block_ids.keys().collect::<Vec<_>>()
        );
    }

    blocks.blocks = maybe_blocks.into_iter().flatten().collect();

    commands.insert_resource(blocks);
}

fn move_blocks_resource_to_static(mut commands: Commands, mut blocks: ResMut<Blocks>) {
    let blocks = std::mem::replace(
        &mut *blocks,
        Blocks {
            blocks: Vec::new(),
            ids: HashMap::new(),
        },
    );
    BLOCKS.set(blocks).ok();
    commands.remove_resource::<Blocks>();
}

// TODO: Loading needs to be done when validating the resources too. Store them?
fn load_block_materials() -> HashMap<String, BlockMaterial> {
    let mut materials = HashMap::new();

    let dir = std::path::PathBuf::from(BLOCK_MATERIAL_PATH);
    for dir_entry in std::fs::read_dir(&dir).unwrap() {
        let file_path = match dir_entry {
            Ok(p) => p.path(),
            Err(e) => panic!(
                "Failed to read block materials from: '{}'\n Make sure the directory is present.\n
                Error: {}",
                BLOCK_MATERIAL_PATH, e
            ),
        };

        let material_name = file_path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => panic!(
                "Failed to open block material config.\nPath: {}\nError: {}",
                file_path.to_string_lossy(),
                e
            ),
        };

        let block_material: BlockMaterial = match serde_json::from_reader(file) {
            Ok(c) => c,
            Err(e) => panic!(
                "Failed to read material configuration at path: '{}'\nError: {}",
                file_path.to_string_lossy(),
                e
            ),
        };

        materials.insert(material_name, block_material);
    }

    return materials;
}

// TODO: This wraps BlockConfig for no good reason? Include spawn_entity_fn in BlockConfig. The
// name can be used for { BlockId, Option<BlockState> }?
#[derive(Deref)]
pub struct Block {
    #[deref]
    config: BlockConfig,
    // This function is used to set up the ecs entity for the block if it should have
    // functionality. e.g. a furnace needs ui components and its internal smelting state.
    pub spawn_entity_fn: Option<fn(&mut EntityCommands, Option<&BlockData>)>,
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("Block")
            .field("config", &self.config)
            .finish()
    }
}

impl Block {
    fn new(config: BlockConfig) -> Self {
        return Self {
            config,
            spawn_entity_fn: None,
        };
    }

    pub fn set_spawn_function(&mut self, function: fn(&mut EntityCommands, Option<&BlockData>)) {
        self.spawn_entity_fn = Some(function);
    }
}

/// The configurations and ids of the blocks in the game.
#[derive(Resource, Debug)]
pub struct Blocks {
    // block id -> block config
    blocks: Vec<Block>,
    // block name -> block id
    ids: HashMap<String, BlockId>,
}

impl Blocks {
    /// WARN: Panics if used during startup. If you need it during startup, use the Resource
    /// instead.
    #[track_caller]
    pub fn get() -> &'static Self {
        BLOCKS.get().unwrap()
    }

    // TODO: Better ergonomics if this doesn't take a reference?
    pub fn get_config(&self, block_id: &BlockId) -> &Block {
        return &self.blocks[*block_id as usize];
    }

    pub fn get_config_mut(&mut self, block_id: &BlockId) -> &mut Block {
        return &mut self.blocks[*block_id as usize];
    }

    #[track_caller]
    pub fn get_id(&self, block_name: &str) -> BlockId {
        match self.ids.get(block_name) {
            Some(b) => *b,
            None => panic!("No block with name '{}'", block_name),
        }
    }

    pub fn asset_ids(&self) -> HashMap<String, BlockId> {
        return self.ids.clone();
    }

    pub fn contains_block(&self, block_name: &str) -> bool {
        return self.ids.contains_key(block_name);
    }
}

#[derive(Debug, Deserialize)]
struct BlockDropJson {
    #[serde(default)]
    requires_tool: bool,
    drop: BlockDropKindJson,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BlockDropKindJson {
    Single(String),
    Multiple { item: String, count: u32 },
    Chance(Vec<(f64, Self)>),
}

#[derive(Debug, Clone)]
struct BlockDrop {
    requires_tool: bool,
    drop: BlockDropKind,
}

impl BlockDrop {
    fn drop(&self, with_tool: bool) -> Option<(ItemId, u32)> {
        if self.requires_tool && !with_tool {
            return None;
        } else {
            return Some(self.drop.drop());
        }
    }
}

#[derive(Debug, Clone)]
enum BlockDropKind {
    Single(ItemId),
    Multiple {
        item: ItemId,
        count: u32,
    },
    // TODO: There's no way to define something that drops only one thing n% of the time.
    Chance {
        // The probablities of the drops.
        weights: WeightedIndex<f64>,
        drops: Vec<Self>,
    },
}

impl BlockDropKind {
    fn from_json(
        json: &BlockDropKindJson,
        items: &HashMap<String, ItemId>,
    ) -> Result<BlockDropKind, String> {
        match json {
            BlockDropKindJson::Single(item_name) => match items.get(item_name) {
                Some(id) => Ok(Self::Single(*id)),
                None => Err(format!("No item by the name {}", item_name)),
            },
            BlockDropKindJson::Multiple { item, count } => match items.get(item) {
                Some(id) => Ok(Self::Multiple {
                    item: *id,
                    count: *count,
                }),
                None => Err(format!("No item by the name {}", item)),
            },
            BlockDropKindJson::Chance(list) => {
                let mut weights = Vec::with_capacity(list.len());
                let mut drops = Vec::with_capacity(list.len());

                for (weight, drop_json) in list {
                    weights.push(weight);
                    let drop = Self::from_json(drop_json, items)?;
                    drops.push(drop);
                }

                let weights = match WeightedIndex::new(weights) {
                    Ok(w) => w,
                    Err(_) => return Err("Weights must be positive and above zero.".to_owned()),
                };

                Ok(Self::Chance { weights, drops })
            }
        }
    }

    fn drop(&self) -> (ItemId, u32) {
        match &self {
            BlockDropKind::Single(item) => (*item, 1),
            BlockDropKind::Multiple { item, count } => (*item, *count),
            BlockDropKind::Chance { weights, drops } => {
                drops[weights.sample(&mut rand::thread_rng())].drop()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct BlockConfigJson {
    // Name of the block
    name: String,
    // The friction/drag.
    friction: Friction,
    // How long it takes to break the block without a tool
    hardness: Option<f32>,
    // Which tool categories will break this block faster.
    #[serde(default)]
    tools: HashSet<String>,
    // Which item(s) the block drops
    drop: Option<BlockDropJson>,
    #[serde(default)]
    is_rotatable: bool,
    // Renderding material, used to deduce transparency.
    // None if it's a model block, the transparency is set to true.
    // If the string is not "opaque", the transparency is set to true.
    material: Option<String>,
}

impl BlockConfigJson {
    fn from_file(path: &Path) -> Option<Self> {
        fn read_as_json_value(
            path: &std::path::Path,
        ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            let file = std::fs::File::open(&path)?;

            let mut config: serde_json::Value = serde_json::from_reader(&file)?;

            // recursively read parent configs
            if let Some(parent) = config["parent"].as_str() {
                let parent_path = std::path::Path::new(BLOCK_CONFIG_PATH).join(parent);
                let mut parent: serde_json::Value = match read_as_json_value(&parent_path) {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(format!(
                            "Failed to read parent block config at {}: {}",
                            parent_path.display(),
                            e
                        )
                        .into())
                    }
                };
                parent
                    .as_object_mut()
                    .unwrap()
                    .append(&mut config.as_object_mut().unwrap());

                config = parent;
            }

            Ok(config)
        }

        let json = match read_as_json_value(path) {
            Ok(j) => j,
            Err(e) => panic!("Failed to read block config at {}: {}", path.display(), e),
        };

        // This filters out parent configs
        if json.get("name").is_some_and(|name| name.is_string()) {
            // TODO: When this fails, theres no way to know which field made it panic.
            return match serde_json::from_value(json) {
                Ok(b) => Some(b),
                Err(e) => panic!("Failed to read block config at {}: {}", path.display(), e),
            };
        } else {
            return None;
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockConfig {
    /// Name of the block
    pub name: String,
    /// The friction or drag.
    pub friction: Friction,
    /// How long it takes to break the block without a tool, None if the block should not be
    /// breakable. e.g. water, air
    pub hardness: Option<f32>,
    // Which tool categories will break this block faster.
    tools: HashSet<String>,
    // Which item(s) the block drops.
    drop: Option<BlockDrop>,
    /// If the block is rotatable around the y axis
    pub is_rotatable: bool,
    /// If the block can be seen through
    pub is_transparent: bool,
}

impl BlockConfig {
    pub fn drop(&self, tool: Option<&str>) -> Option<(ItemId, u32)> {
        let Some(block_drop) = &self.drop else {
            return None;
        };
        if let Some(tool) = tool {
            return block_drop.drop(self.tools.contains(tool));
        } else {
            return block_drop.drop(false);
        }
    }

    pub fn is_solid(&self) -> bool {
        match self.friction {
            Friction::Static { .. } => true,
            Friction::Drag(_) => false,
        }
    }
}

/// The different sides of a block
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum BlockFace {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
}

impl BlockFace {
    pub fn shift_position(&self, position: IVec3) -> IVec3 {
        match self {
            Self::Front => position + IVec3::Z,
            Self::Back => position - IVec3::Z,
            Self::Right => position + IVec3::X,
            Self::Left => position - IVec3::X,
            Self::Top => position + IVec3::Y,
            Self::Bottom => position - IVec3::Y,
        }
    }

    pub fn to_rotation(&self) -> BlockRotation {
        match self {
            Self::Front => BlockRotation::None,
            Self::Right => BlockRotation::Once,
            Self::Back => BlockRotation::Twice,
            Self::Left => BlockRotation::Thrice,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Friction {
    /// Friction for solid blocks.
    Static {
        front: f64,
        back: f64,
        right: f64,
        left: f64,
        top: f64,
        bottom: f64,
    },
    /// For non-collidable blocks, the friction is instead drag on the player movement.
    Drag(DVec3),
}

#[derive(Component, Deref, DerefMut)]
pub struct BlockData(pub Vec<u8>);

#[derive(Default, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct BlockState(pub u16);

impl BlockState {
    pub fn new(rotation: BlockRotation) -> Self {
        return BlockState(rotation as u16);
    }

    pub fn as_u16(&self) -> u16 {
        return self.0;
    }

    pub fn rotation(&self) -> BlockRotation {
        return BlockRotation::from(self.0);
    }

    pub fn set_rotation(&mut self, rotation: BlockRotation) {
        self.0 = self.0 & !0b11 & (rotation as u16 & 0b11);
    }
}

#[derive(Component, Deref, DerefMut)]
pub struct BlockPosition(pub IVec3);

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum BlockRotation {
    None = 0,
    Once,
    Twice,
    Thrice,
}

impl From<u16> for BlockRotation {
    #[track_caller]
    fn from(value: u16) -> Self {
        return unsafe { std::mem::transmute(value & 0b11) };
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct BlockMaterial {
    transparency: String,
}

impl Default for BlockMaterial {
    fn default() -> Self {
        Self {
            transparency: "opaque".to_owned(),
        }
    }
}
