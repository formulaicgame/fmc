// TODO: It should store block configs in the worlds database so that worlds are more portable.
//       Addendum: It should store the entire resource folder.
//       It should instead emit warnings when configs(and other things it was initialized with) go
//       missing, and update the database if a config has been changed.
use std::{
    collections::{HashMap, HashSet},
    ops::{Add, AddAssign, Sub, SubAssign},
    path::Path,
};

use bevy::{
    ecs::system::EntityCommands,
    math::{DQuat, DVec3},
};
use rand::{distributions::WeightedIndex, prelude::Distribution};
use serde::Deserialize;

use crate::{
    assets::AssetSet,
    database::Database,
    items::{ItemConfig, ItemId, Items},
    models::{ModelId, Models},
    physics::{shapes::Aabb, Collider},
    prelude::*,
    utils::Rng,
    world::chunk::{Chunk, ChunkPosition},
};

pub type BlockId = u16;

pub const BLOCK_CONFIG_PATH: &str = "./assets/client/blocks/";
const BLOCK_MATERIAL_PATH: &str = "./assets/client/materials/";

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
        app.add_systems(PreStartup, load_blocks_to_resource.in_set(AssetSet::Blocks))
            .add_systems(PostStartup, move_blocks_resource_to_static);
    }
}

fn load_blocks_to_resource(
    mut commands: Commands,
    database: Res<Database>,
    models: Res<Models>,
    items: Res<Items>,
) {
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

    let mut block_ids = blocks.asset_ids();
    let mut maybe_blocks = Vec::new();
    maybe_blocks.resize_with(block_ids.len(), Option::default);

    let block_materials = load_block_materials();

    for file_path in walk_dir(&BLOCK_CONFIG_PATH) {
        let block_config_json = match BlockConfigJson::from_file(&file_path) {
            Some(b) => b,
            None => continue,
        };

        let drop = match block_config_json.drop {
            Some(drop) => {
                let kind = match BlockDropKind::from_json(&drop.drop, &items) {
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

        let material = if let Some(material_name) = &block_config_json.material {
            block_materials.get(material_name).cloned()
        } else {
            None
        };

        let model_id = if let Some(model_name) = &block_config_json.model {
            Some(models.get_by_name(&model_name).id)
        } else {
            None
        };

        let hitbox = if let Some(hitbox) = block_config_json.hitbox {
            Some(hitbox.to_collider())
        } else if let Some(model_name) = block_config_json.model {
            let model_config = models.get_by_name(&model_name);
            let aabb = model_config.aabb.clone();
            Some(Collider::Aabb(aabb))
        } else if block_config_json.faces.is_some() {
            let aabb = Aabb::from_min_max(DVec3::ZERO, DVec3::ONE);
            Some(Collider::Aabb(aabb))
        } else if let Some(quads) = block_config_json.quads {
            let mut min = Vec3::MAX;
            let mut max = Vec3::MIN;
            for quad in quads {
                for vertex in quad.vertices.map(Vec3::from) {
                    min = min.min(vertex);
                    max = max.max(vertex);
                }
            }
            let aabb = Aabb::from_min_max(min.as_dvec3(), max.as_dvec3());
            Some(Collider::Aabb(aabb))
        } else {
            None
        };

        let particle_textures = if let Some(particle_texture) = block_config_json.particle_texture {
            Some(BlockFaceTextures {
                top: particle_texture.clone(),
                bottom: particle_texture.clone(),
                right: particle_texture.clone(),
                left: particle_texture.clone(),
                front: particle_texture.clone(),
                back: particle_texture,
            })
        } else if let Some(faces) = block_config_json.faces {
            // The path must be relative to /textures/ but faces are specified relative to
            // /textures/blocks
            let path = "blocks/";
            Some(BlockFaceTextures {
                top: path.to_owned() + &faces.top,
                bottom: path.to_owned() + &faces.bottom,
                right: path.to_owned() + &faces.right,
                left: path.to_owned() + &faces.left,
                front: path.to_owned() + &faces.front,
                back: path.to_owned() + &faces.back,
            })
        } else {
            None
        };

        if let Some(block_id) = block_ids.remove(&block_config_json.name) {
            let block_config = BlockConfig {
                name: block_config_json.name,
                model: model_id,
                friction: block_config_json.friction,
                drag: block_config_json.drag,
                hardness: block_config_json.hardness,
                replaceable: block_config_json.replaceable,
                tools: block_config_json.tools,
                drop,
                material,
                placement: block_config_json.placement,
                hitbox,
                particle_textures,
                sound: block_config_json.sound,
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
    let blocks = std::mem::replace(&mut *blocks, Blocks::default());
    BLOCKS.set(blocks).ok();
    commands.remove_resource::<Blocks>();
}

// TODO: Loading needs to be done when validating the assets too. Store them?
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
#[derive(Resource, Debug, Default)]
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
    Multiple { item_name: String, count: u32 },
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
        item_id: ItemId,
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
    fn from_json(json: &BlockDropKindJson, items: &Items) -> Result<BlockDropKind, String> {
        match json {
            BlockDropKindJson::Single(item_name) => match items.get_id(item_name) {
                Some(id) => Ok(Self::Single(id)),
                None => Err(format!("No item by the name {}", item_name)),
            },
            BlockDropKindJson::Multiple { item_name, count } => match items.get_id(item_name) {
                Some(item_id) => Ok(Self::Multiple {
                    item_id,
                    count: *count,
                }),
                None => Err(format!("No item by the name {}", item_name)),
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
            BlockDropKind::Single(item_id) => (*item_id, 1),
            BlockDropKind::Multiple { item_id, count } => (*item_id, *count),
            BlockDropKind::Chance { weights, drops } => {
                drops[weights.sample(&mut rand::thread_rng())].drop()
            }
        }
    }
}

// Paths to textures used by a cube relative to /textures/
#[derive(Debug, Deserialize, Clone)]
struct BlockFaceTextures {
    top: String,
    bottom: String,
    left: String,
    right: String,
    front: String,
    back: String,
}

#[derive(Debug, Deserialize)]
struct AabbJson {
    min: DVec3,
    max: DVec3,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ColliderJson {
    Aabb(AabbJson),
    Compound(Vec<AabbJson>),
}

impl ColliderJson {
    fn to_collider(&self) -> Collider {
        match self {
            ColliderJson::Aabb(aabb) => Collider::Aabb(Aabb::from_min_max(aabb.min, aabb.max)),
            ColliderJson::Compound(list) => Collider::Compound(
                list.into_iter()
                    .map(|aabb| Aabb::from_min_max(aabb.min, aabb.max))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BlockVerticesJson {
    vertices: [[f32; 3]; 4],
}

#[derive(Debug, Deserialize, Default)]
pub struct Sounds {
    #[serde(default)]
    place: Vec<String>,
    #[serde(default)]
    step: Vec<String>,
    #[serde(default)]
    hit: Vec<String>,
    #[serde(default)]
    destroy: Vec<String>,
}

impl Sounds {
    pub fn step(&self, rng: &mut Rng) -> Option<&str> {
        if self.step.len() == 0 {
            return None;
        }

        Some(&self.step[rng.next_u32() as usize % self.step.len()])
    }

    pub fn hit(&self, rng: &mut Rng) -> Option<&str> {
        if self.hit.len() == 0 {
            return None;
        }

        Some(&self.hit[rng.next_u32() as usize % self.hit.len()])
    }

    pub fn destroy(&self, rng: &mut Rng) -> Option<&str> {
        if self.destroy.len() == 0 {
            return None;
        }

        Some(&self.destroy[rng.next_u32() as usize % self.destroy.len()])
    }

    pub fn place(&self, rng: &mut Rng) -> Option<&str> {
        if self.place.len() == 0 {
            return None;
        }

        Some(&self.place[rng.next_u32() as usize % self.place.len()])
    }
}

#[derive(Debug, Deserialize)]
struct BlockConfigJson {
    // Name of the block
    name: String,
    // The surface friction.
    friction: Option<Friction>,
    // The drag when inside the block
    #[serde(default)]
    drag: DVec3,
    // How long it takes to break the block without a tool
    hardness: Option<f32>,
    #[serde(default)]
    replaceable: bool,
    // Which tool categories will break this block faster.
    #[serde(default)]
    tools: HashSet<String>,
    // Which item(s) the block drops
    drop: Option<BlockDropJson>,
    // Renderding material name, used to get the transparency.
    // None if it's a model block and the transparency is set to true.
    material: Option<String>,
    // Collider used for physics/hit detection.
    hitbox: Option<ColliderJson>,
    // These are the three ways you can define a block. We use them to generate the hitbox when it
    // is not explicitly defined. 'model' is a gltf model, 'quads' is a set vertices and 'faces' is
    // the six faces of a cube.
    model: Option<String>,
    quads: Option<Vec<BlockVerticesJson>>,
    faces: Option<BlockFaceTextures>,
    // Rules for how the block can be placed by the player.
    #[serde(default)]
    placement: BlockPlacement,
    // Texture used for particle when brekaing the block. Relative to /textures/
    // If not supplied it will be derived from 'faces' if that is supplied.
    particle_texture: Option<String>,
    #[serde(default)]
    sound: Sounds,
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

// TODO: 'hardness' 'tools' 'drop' 'particle_textures' are too specific. They should be handled
// outside of library. Add a new field 'properties' with serde(flatten) on it to capture everything
// not needed. The server implementor should then make their own 'Blocks' and 'BlockConfig'. Parse
// the 'properties' field into it's BlockConfig and shadow Blocks.
#[derive(Debug)]
pub struct BlockConfig {
    /// Name of the block
    pub name: String,
    /// If a model is used to represent this block, this contains its model id
    pub model: Option<ModelId>,
    /// The friction of the block's surfaces.
    pub friction: Option<Friction>,
    /// The frictional drag when inside the block.
    pub drag: DVec3,
    // TODO: Not needed
    /// How long it takes to break the block without a tool in seconds, None if the block should
    /// not be breakable. e.g. water, air
    pub hardness: Option<f32>,
    /// Makes it possible to replace the block by placing another in its position.
    pub replaceable: bool,
    // TODO: Not needed
    // Which tool categories will break this block faster.
    pub tools: HashSet<String>,
    // TODO: Not needed
    // Which item(s) the block drops.
    drop: Option<BlockDrop>,
    // The rendering material for the block, if it uses one.
    pub material: Option<BlockMaterial>,
    /// Collider used for physics and hit detection.
    pub hitbox: Option<Collider>,
    /// Rules for how the block can be placed by the player.
    pub placement: BlockPlacement,
    // TODO: Not needed
    /// Texture path for each face used for particles when breaking blocks
    particle_textures: Option<BlockFaceTextures>,
    // TODO: Not needed
    /// Sound files associated with the block
    pub sound: Sounds,
}

impl BlockConfig {
    pub fn is_transparent(&self) -> bool {
        if let Some(material) = &self.material {
            if material.transparency == "opaque" {
                false
            } else {
                true
            }
        } else {
            true
        }
    }

    pub fn drop(&self, tool: Option<&ItemConfig>) -> Option<(ItemId, u32)> {
        let Some(block_drop) = &self.drop else {
            return None;
        };

        if let Some(tool) = tool.and_then(|t| t.tool.as_ref()) {
            return block_drop.drop(self.tools.contains(&tool.name));
        } else {
            return block_drop.drop(false);
        }
    }

    pub fn is_solid(&self) -> bool {
        self.friction.is_some()
    }

    pub fn is_placeable(&self, against_block_face: BlockFace) -> bool {
        match against_block_face {
            BlockFace::Bottom if self.placement.ceiling => true,
            BlockFace::Top if self.placement.floor => true,
            BlockFace::Right | BlockFace::Left | BlockFace::Front | BlockFace::Back
                if self.placement.sides =>
            {
                true
            }

            _ => false,
        }
    }

    pub fn placement_rotation(&self, against_block_face: BlockFace) -> Option<BlockState> {
        if !self.is_placeable(against_block_face) {
            return None;
        }

        if !self.placement.rotatable {
            return None;
        }

        let mut block_state = BlockState::new();

        if (against_block_face == BlockFace::Bottom || against_block_face == BlockFace::Top)
            && self.placement.centered
        {
            block_state.set_centered(true);
        } else {
            block_state.set_rotation(against_block_face.to_rotation());
        }

        return Some(block_state);
    }

    pub fn particle_texture(&self, block_face: BlockFace) -> Option<&str> {
        if let Some(paths) = &self.particle_textures {
            let path = match block_face {
                BlockFace::Top => &paths.top,
                BlockFace::Bottom => &paths.bottom,
                BlockFace::Right => &paths.right,
                BlockFace::Left => &paths.left,
                BlockFace::Front => &paths.front,
                BlockFace::Back => &paths.back,
            };
            Some(path)
        } else {
            None
        }
    }

    pub fn particle_color(&self) -> Option<String> {
        let Some(material) = &self.material else {
            return None;
        };

        let Some(color) = &material.base_color else {
            return None;
        };

        let r = (color.red.clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (color.green.clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (color.blue.clamp(0.0, 1.0) * 255.0).round() as u8;
        let a = (color.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;

        return Some(format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a));
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BlockPlacement {
    /// Set if the block can be placed by clicking the top face of a block
    pub floor: bool,
    /// Set if the block can be placed by clicking the bottom face of a block
    pub ceiling: bool,
    /// Set if the block can be placed by clicking the sides of blocks
    pub sides: bool,
    /// Set if the block should be rotated when placed.
    pub rotatable: bool,
    /// If 'rotatable' is set, this allows a block to be placed without rotation if it is placed on
    /// the Top or Bottom face of a block.
    pub centered: bool,
    /// Set if a transform should be applied when rotated.
    pub rotation_transform: Option<Transform>,
}

impl Default for BlockPlacement {
    fn default() -> Self {
        Self {
            floor: true,
            ceiling: true,
            sides: true,
            rotatable: false,
            centered: true,
            rotation_transform: None,
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
    pub fn shift_position(&self, position: BlockPosition) -> BlockPosition {
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
            Self::Front => BlockRotation::Front,
            Self::Right => BlockRotation::Right,
            Self::Back => BlockRotation::Back,
            Self::Left => BlockRotation::Left,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Friction {
    pub front: f64,
    pub back: f64,
    pub right: f64,
    pub left: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Component, Deref, DerefMut)]
pub struct BlockData(pub Vec<u8>);

// bits:
//     0000 0000 0000 unused
//     0000
//       ^^-north/south/east/west
//      ^---centered, overrides rotation, 1 = centered
//     ^----upside down
#[derive(Default, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub struct BlockState(pub u16);

impl BlockState {
    pub fn new() -> Self {
        return Self(0);
    }

    pub fn as_u16(self) -> u16 {
        return self.0;
    }

    pub fn set_centered(&mut self, centered: bool) {
        self.0 &= !0b100;
        self.0 |= (centered as u16) << 2;
    }

    pub fn is_centered(&self) -> bool {
        self.0 & 0b100 != 0
    }

    pub fn set_rotation(&mut self, rotation: BlockRotation) {
        self.0 &= !0b11;
        self.0 |= rotation as u16;
    }

    pub fn with_rotation(mut self, rotation: BlockRotation) -> Self {
        self.set_rotation(rotation);
        self
    }

    pub fn rotation(self) -> Option<BlockRotation> {
        if self.is_centered() {
            return None;
        }

        if self.0 & 0b100 == 0 {
            return Some(BlockRotation::from(self.0));
        } else {
            None
        }
    }
}

// TODO: Replace all occurences of IVec3 with this
#[derive(Component, Deref, DerefMut, Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct BlockPosition(pub IVec3);

impl BlockPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub fn as_chunk_index(&self) -> usize {
        // Getting the last 4 bits will output 0->Chunk::SIZE for both positive and negative numbers
        // because of two's complement.
        let position = self.0 & (Chunk::SIZE - 1) as i32;
        return (position.x << 8 | position.z << 4 | position.y) as usize;
    }
}

impl From<DVec3> for BlockPosition {
    fn from(value: DVec3) -> Self {
        Self(value.floor().as_ivec3())
    }
}

impl From<usize> for BlockPosition {
    fn from(index: usize) -> Self {
        assert!(index < Chunk::SIZE.pow(3));
        const MASK: usize = Chunk::SIZE - 1;
        BlockPosition(IVec3 {
            x: index as i32 >> 8,
            z: (index >> 4 & MASK) as i32,
            y: (index & MASK) as i32,
        })
    }
}

impl From<ChunkPosition> for BlockPosition {
    fn from(chunk_position: ChunkPosition) -> Self {
        BlockPosition(chunk_position.0)
    }
}

impl Add<BlockPosition> for BlockPosition {
    type Output = BlockPosition;

    fn add(self, rhs: BlockPosition) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<IVec3> for BlockPosition {
    type Output = BlockPosition;

    fn add(self, rhs: IVec3) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<BlockPosition> for BlockPosition {
    #[inline]
    fn add_assign(&mut self, rhs: BlockPosition) {
        self.0.add_assign(rhs.0);
    }
}

impl AddAssign<IVec3> for BlockPosition {
    #[inline]
    fn add_assign(&mut self, rhs: IVec3) {
        self.0.add_assign(rhs);
    }
}

impl Sub<BlockPosition> for BlockPosition {
    type Output = BlockPosition;

    fn sub(self, rhs: BlockPosition) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<IVec3> for BlockPosition {
    type Output = BlockPosition;

    fn sub(self, rhs: IVec3) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl SubAssign<BlockPosition> for BlockPosition {
    #[inline]
    fn sub_assign(&mut self, rhs: BlockPosition) {
        self.0.sub_assign(rhs.0);
    }
}

impl SubAssign<IVec3> for BlockPosition {
    #[inline]
    fn sub_assign(&mut self, rhs: IVec3) {
        self.0.sub_assign(rhs);
    }
}

/// The rotation of a block, the variants correspond to which way the block will face when rotated,
/// but it is still a CCW rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockRotation {
    Front = 0,
    Right,
    Back,
    Left,
}

impl From<u16> for BlockRotation {
    #[track_caller]
    fn from(value: u16) -> Self {
        return unsafe { std::mem::transmute(value & 0b11) };
    }
}

impl BlockRotation {
    pub fn as_quat(self) -> DQuat {
        match self {
            Self::Front => DQuat::from_rotation_y(0.0),
            Self::Right => DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
            Self::Back => DQuat::from_rotation_y(std::f64::consts::PI),
            Self::Left => DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_2),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct Color {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BlockMaterial {
    base_color: Option<Color>,
    transparency: String,
}

impl Default for BlockMaterial {
    fn default() -> Self {
        Self {
            base_color: None,
            transparency: "opaque".to_owned(),
        }
    }
}
