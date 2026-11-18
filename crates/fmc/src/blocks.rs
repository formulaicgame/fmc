// TODO: It should store block configs in the worlds database so that worlds are more portable.
//       Addendum: It should store the entire resource folder.
//       It should instead emit warnings when configs(and other things it was initialized with) go
//       missing, and update the database if a config has been changed.
use std::{
    collections::HashMap,
    ops::{Add, AddAssign, Sub, SubAssign},
    path::Path,
};

use crate::bevy::{
    ecs::system::EntityCommands,
    math::{DQuat, DVec3},
};
use serde::Deserialize;

use crate::{
    assets::AssetSet,
    database::Database,
    models::{ModelAssetId, Models},
    particle_effects::{ParticleEffects, ParticleSpawner},
    physics::{Collider, ColliderJson, Friction, shapes::Aabb},
    players::Camera,
    prelude::*,
    world::chunk::{Chunk, ChunkPosition},
};

/// Each block type is assiged a unique block id
pub type BlockId = u16;

/// The relative path to the game's block configurations.
pub const BLOCK_CONFIG_PATH: &str = "./assets/client/blocks/";
/// The relative path to the game's rendering materials
pub const BLOCK_MATERIAL_PATH: &str = "./assets/client/materials/block";

// TODO: Blocks needs to be a static because it is used during Chunk::load, which is part of the
// public api, so we can't pass a private struct to it as an argument. Not the end of the world
// and it mirrors Items which does the same for a different reason, but it is not pretty.
static BLOCKS: once_cell::sync::OnceCell<Blocks> = once_cell::sync::OnceCell::new();

/// Handles loading block from their configuration files
pub struct BlockPlugin;
impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockConfigEvent>()
            .add_systems(PreStartup, load_blocks.in_set(AssetSet::Blocks));
    }
}

/// Contains the parsed json of a block. Use it to extract unique configuration values.
/// Sent during PreStartup and gone after PostStartup
#[derive(Message, Clone, Debug)]
pub struct BlockConfigEvent {
    pub block_id: BlockId,
    pub name: String,
    pub json: serde_json::Value,
}

type SpawnFunction = fn(&mut EntityCommands, Option<&BlockData>);

#[derive(Resource)]
pub struct BlockSpawnFunctions(Vec<Option<SpawnFunction>>);

impl BlockSpawnFunctions {
    pub fn set(&mut self, block_id: BlockId, function: SpawnFunction) {
        self.0[block_id as usize] = Some(function);
    }

    pub(crate) fn get(&self, block_id: &BlockId) -> Option<SpawnFunction> {
        return self.0[*block_id as usize];
    }
}

fn load_blocks(
    mut commands: Commands,
    database: Res<Database>,
    models: Res<Models>,
    particle_effects: Res<ParticleEffects>,
    mut block_config_events: MessageWriter<BlockConfigEvent>,
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

    let mut block_ids = blocks.ids().clone();
    let mut maybe_blocks = Vec::new();
    maybe_blocks.resize_with(block_ids.len(), Option::default);

    let block_materials = load_block_materials();

    for file_path in walk_dir(&BLOCK_CONFIG_PATH) {
        let Some(json) = read_block_json(&file_path) else {
            continue;
        };

        // TODO: When this fails, theres no way to know which field made it panic.
        let block_config_json = match BlockConfigJson::deserialize(&json) {
            Ok(b) => b,
            Err(e) => panic!(
                "Failed to read block config at {}: {}",
                file_path.display(),
                e
            ),
        };

        let material = if let Some(material_name) = &block_config_json.material {
            block_materials.get(material_name).cloned()
        } else {
            None
        };

        let model_id = if let Some(model_name) = &block_config_json.model {
            match models.get_config_by_name(&model_name) {
                Some(m) => Some(m.id),
                None => {
                    panic!(
                        "Failed to read block at: {}\nError: no model by the name: {}",
                        file_path.display(),
                        model_name
                    );
                }
            }
        } else {
            None
        };

        let particle_spawners: Vec<BlockParticleSpawner> = block_config_json
            .particle_spawners
            .iter()
            .map(|spawner| {
                let Some(effect) = particle_effects.get_id(&spawner.effect) else {
                    panic!(
                        "Failed to read block at: {}\nError: no particle effect by the name: {}",
                        file_path.display(),
                        spawner.effect
                    );
                };

                let [min, max] = spawner.interval;
                if !(min > 0.0 && max >= min) {
                    panic!(
                        "Failed to read block at: {}\nError: the particle spawner interval \
                        [{}, {}] must be positive and ordered from min to max",
                        file_path.display(),
                        min,
                        max
                    );
                }

                BlockParticleSpawner {
                    spawner: ParticleSpawner {
                        effect,
                        texture: spawner.texture.clone(),
                        color: spawner.color,
                        interval: spawner.interval,
                    },
                    offset: spawner.offset.as_dvec3() - DVec3::new(0.5, 0.0, 0.5),
                }
            })
            .collect();

        let collider = match Collider::from_block_json(&json, &models) {
            Ok(c) => c,
            Err(e) => panic!(
                "Failed to read block at: {}\nError: {}",
                file_path.display(),
                e
            ),
        };

        if let Some(block_id) = block_ids.remove(&block_config_json.name) {
            block_config_events.write(BlockConfigEvent {
                block_id,
                name: block_config_json.name,
                json,
            });

            let block_config = BlockConfig {
                model: model_id,
                friction: block_config_json
                    .friction
                    .unwrap_or(Friction::Drag(DVec3::ZERO)),
                force: block_config_json.force.unwrap_or_default(),
                replaceable: block_config_json.replaceable,
                transparent: material
                    .as_ref()
                    .is_none_or(|material| material.transparency != "opaque"),
                rotation_transform: block_config_json.rotation_transform,
                particle_spawners,
                collider,
            };

            maybe_blocks[block_id as usize] = Some(block_config);
        }
    }

    if block_ids.len() > 0 {
        panic!(
            "Misconfigured resource pack, missing blocks: {:?}",
            block_ids.keys().collect::<Vec<_>>()
        );
    }

    blocks.blocks = maybe_blocks.into_iter().flatten().collect();

    commands.insert_resource(BlockSpawnFunctions(
        blocks.blocks.iter().map(|_| None).collect(),
    ));

    BLOCKS.set(blocks).ok();
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

        let contents = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => panic!(
                "Failed to open block material config.\nPath: {}\nError: {}",
                file_path.to_string_lossy(),
                e
            ),
        };

        let block_material: BlockMaterial = match serde_json::from_str(&contents) {
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

// TODO: The original intention was to make this public so consumers wouldn't have to worry about
// parsing the blocks, but all the ways I can come up with for parsing arbitrary fields are bad.
// This might still be useful for "infrastructure" plugins, in which case it would have to be made
// public under some different name so it doesn't collide, BlockRegistry perhaps.
//
/// The configurations and ids of the blocks in the game.
#[derive(Debug)]
pub(crate) struct Blocks {
    // block id -> block config
    blocks: Vec<BlockConfig>,
    // block name -> block id
    ids: HashMap<String, BlockId>,
}

impl Blocks {
    #[track_caller]
    pub fn get() -> &'static Self {
        BLOCKS.get().unwrap()
    }

    // TODO: Better ergonomics if this doesn't take a reference?
    pub fn get_config(&self, block_id: &BlockId) -> &BlockConfig {
        return &self.blocks[*block_id as usize];
    }

    pub fn ids(&self) -> &HashMap<String, BlockId> {
        return &self.ids;
    }
}

#[derive(Debug, Deserialize)]
pub struct BlockQuad {
    pub vertices: [[f32; 3]; 4],
}

#[derive(Debug, Deserialize)]
struct BlockConfigJson {
    // Name of the block
    name: String,
    // The surface friction or drag.
    friction: Option<Friction>,
    force: Option<DVec3>,
    #[serde(default)]
    replaceable: bool,
    // Renderding material name, used to get the transparency.
    // None if it's a model block and the transparency is set to true.
    material: Option<String>,
    model: Option<String>,
    // Transform applied to the model when the block is rotated.
    rotation_transform: Option<Transform>,
    // Particle spawners attached to the block.
    #[serde(default)]
    particle_spawners: Vec<ParticleSpawnerJson>,
}

fn read_block_json(path: &Path) -> Option<serde_json::Value> {
    fn read_as_json_value(
        path: &std::path::Path,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(&path)?;

        let mut config: serde_json::Value = serde_json::from_str(&contents)?;

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
                    .into());
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

    // Filter out parent configs
    if json.get("name").is_some_and(|name| name.is_string()) {
        return Some(json);
    } else {
        return None;
    }
}

impl Collider {
    pub fn from_block_json(json: &serde_json::Value, models: &Models) -> Result<Collider, String> {
        #[derive(Deserialize)]
        struct ColliderSource {
            hitbox: Option<ColliderJson>,
            model: Option<String>,
            quads: Option<Vec<BlockQuad>>,
        }

        let source = match ColliderSource::deserialize(json) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string()),
        };

        // non-model blocks are defined in the 0..1 interval. This makes their collider hard to
        // rotate as they have their center of rotation at Vec3::splat(0.5). We move it to the origin to
        // avoid this. Model blocks don't have this problem as they are defined at the origin.
        let collider = if let Some(hitbox) = source.hitbox {
            match hitbox {
                ColliderJson::Single(aabb) => {
                    Collider::from_min_max(aabb.min - 0.5, aabb.max - 0.5)
                }
                ColliderJson::Vec(aabbs) => Collider::Multi(
                    aabbs
                        .into_iter()
                        .map(|aabb| Aabb::from_min_max(aabb.min - 0.5, aabb.max - 0.5))
                        .collect(),
                ),
            }
        } else if let Some(model_name) = &source.model {
            match models.get_config_by_name(model_name) {
                Some(model_config) => model_config.collider.clone(),
                None => return Err(format!("no model by the name: {}", model_name)),
            }
        } else if let Some(quads) = &source.quads {
            let mut min = Vec3::MAX;
            let mut max = Vec3::MIN;
            for quad in quads {
                for vertex in quad.vertices.map(Vec3::from) {
                    min = min.min(vertex);
                    max = max.max(vertex);
                }
            }
            Collider::from_min_max(min.as_dvec3() - 0.5, max.as_dvec3() - 0.5)
        } else {
            Collider::from_min_max(DVec3::splat(-0.5), DVec3::splat(0.5))
        };

        return Ok(collider);
    }
}

/// The configuration values of a block
#[derive(Debug)]
pub(crate) struct BlockConfig {
    /// If a model is used to represent this block, this contains its model id
    pub model: Option<ModelAssetId>,
    /// The friction of the block's surfaces or its drag.
    pub friction: Friction,
    pub force: DVec3,
    /// Makes it possible to replace the block by placing another in its position.
    pub replaceable: bool,
    // Whether the block can be seen through, derived from the material.
    transparent: bool,
    /// Collider used for physics and hit detection.
    pub collider: Collider,
    /// Transform applied to the model when the block is rotated.
    pub rotation_transform: Option<Transform>,
    /// Particle spawners attached to the block.
    pub particle_spawners: Vec<BlockParticleSpawner>,
}

impl BlockConfig {
    pub fn is_transparent(&self) -> bool {
        return self.transparent;
    }

    pub fn surface_friction(&self, face: BlockFace) -> DVec3 {
        self.friction.surface_friction(face)
    }

    pub fn drag(&self) -> Option<DVec3> {
        self.friction.drag()
    }

    pub fn is_solid(&self) -> bool {
        matches!(self.friction, Friction::Surface { .. })
    }
}

/// Defines how a block can be placed.
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
}

impl Default for BlockPlacement {
    fn default() -> Self {
        Self {
            floor: true,
            ceiling: true,
            sides: true,
            rotatable: false,
            centered: false,
        }
    }
}

impl BlockPlacement {
    pub fn is_placeable(&self, against_block_face: BlockFace) -> bool {
        match against_block_face {
            BlockFace::Bottom if self.ceiling => true,
            BlockFace::Top if self.floor => true,
            BlockFace::Right | BlockFace::Left | BlockFace::Front | BlockFace::Back
                if self.sides =>
            {
                true
            }

            _ => false,
        }
    }

    pub fn placement_rotation(
        &self,
        against_block_face: BlockFace,
        camera: &Camera,
    ) -> Option<BlockState> {
        if !self.is_placeable(against_block_face) {
            return None;
        }

        if !self.rotatable {
            return None;
        }

        let mut block_state = BlockState::new();

        if against_block_face == BlockFace::Bottom || against_block_face == BlockFace::Top {
            if self.centered {
                block_state.set_centered(true);
            } else {
                let facing = camera.forward().xz().as_vec2();
                let rotation = if facing.dot(Vec2::X).abs() > facing.dot(Vec2::Y).abs() {
                    if facing.x.is_sign_positive() {
                        BlockRotation::Left
                    } else {
                        BlockRotation::Right
                    }
                } else {
                    if facing.y.is_sign_positive() {
                        BlockRotation::Back
                    } else {
                        BlockRotation::Front
                    }
                };
                block_state.set_rotation(rotation);
            }
        } else {
            block_state.set_rotation(against_block_face.to_block_rotation());
        }

        return Some(block_state);
    }
}

/// A particle spawner that is spawned together with a block.
#[derive(Debug, Clone)]
pub struct BlockParticleSpawner {
    pub spawner: ParticleSpawner,
    /// Position of the spawner relative to the origin of the block's model.
    pub offset: DVec3,
}

#[derive(Deserialize, Debug)]
struct ParticleSpawnerJson {
    effect: String,
    texture: String,
    #[serde(default = "ParticleSpawnerJson::default_color")]
    color: Vec4,
    interval: [f32; 2],
    #[serde(default = "ParticleSpawnerJson::default_offset")]
    offset: Vec3,
}

impl ParticleSpawnerJson {
    fn default_color() -> Vec4 {
        Vec4::ONE
    }

    fn default_offset() -> Vec3 {
        Vec3::splat(0.5)
    }
}

/// The different faces of a block
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
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            Self::Front,
            Self::Back,
            Self::Right,
            Self::Left,
            Self::Top,
            Self::Bottom,
        ]
        .into_iter()
    }

    pub fn offset(&self) -> IVec3 {
        match self {
            Self::Front => IVec3::Z,
            Self::Back => IVec3::NEG_Z,
            Self::Right => IVec3::X,
            Self::Left => IVec3::NEG_X,
            Self::Top => IVec3::Y,
            Self::Bottom => IVec3::NEG_Y,
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            Self::Front => Self::Back,
            Self::Back => Self::Front,
            Self::Right => Self::Left,
            Self::Left => Self::Right,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }

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

    pub fn to_block_rotation(&self) -> BlockRotation {
        match self {
            Self::Front => BlockRotation::Front,
            Self::Right => BlockRotation::Right,
            Self::Back => BlockRotation::Back,
            Self::Left => BlockRotation::Left,
            _ => unreachable!(),
        }
    }

    pub fn to_quat(&self) -> DQuat {
        match self {
            Self::Front => DQuat::from_rotation_y(0.0),
            Self::Right => DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
            Self::Back => DQuat::from_rotation_y(std::f64::consts::PI),
            Self::Left => DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_2),
            Self::Top => DQuat::IDENTITY,
            Self::Bottom => DQuat::from_rotation_x(std::f64::consts::PI),
        }
    }
}

/// Block data is extra data that is optionally used by blocks that have their own entity. It can
/// be used in any way the implementer chooses, but should be kept as small as possible, as it is
/// unique to each block.
#[derive(Component, Deref, DerefMut, Clone)]
pub struct BlockData(pub Vec<u8>);

// TODO: BlockState is always optional so it has to be unwrapped from Option<BlockState> everywhere.
// This is very cumbersome. Maybe it's a better interface to store the option internally in the struct?
//
/// Common state that can be attached to blocks to alter them. Sent to clients along with the block
/// ids in chunks.  
///
/// bits:
///     0000 0000 0000 unused
///     0000
///       ^^-north/south/east/west
///      ^---centered, overrides rotation, 1 = centered
///     ^----upside down
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

    pub fn rotation(&self) -> Option<BlockRotation> {
        if self.is_centered() {
            return None;
        }

        return Some(BlockRotation::from(*self));
    }
}

/// Required component for all blocks that have entities. Marks where they are in the world.
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

impl From<Vec3> for BlockPosition {
    fn from(value: Vec3) -> Self {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum BlockRotation {
    Front = 0,
    Right,
    Back,
    Left,
}

impl From<BlockState> for BlockRotation {
    #[track_caller]
    fn from(value: BlockState) -> Self {
        return unsafe { std::mem::transmute(value.0 & 0b11) };
    }
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
