use std::{collections::HashMap, path::PathBuf};

use bevy::prelude::*;
use fmc_protocol::messages;
use serde::Deserialize;

use crate::{
    assets,
    networking::NetworkClient,
    rendering::materials::{self, BlockMaterial},
};

pub type BlockId = u16;
pub static mut BLOCKS: std::sync::OnceLock<Blocks> = std::sync::OnceLock::new();

const MODEL_PATH: &str = "server_assets/active/textures/models/";
const BLOCK_CONFIG_PATH: &str = "server_assets/active/blocks/";

const FACE_VERTICES: [[[f32; 3]; 4]; 6] = [
    // Top
    [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
    ],
    // Back
    [
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
    // Left
    [
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
    ],
    // Right
    [
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    ],
    // Front
    [
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
    ],
    // Bottom
    [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
    ],
];

const FACE_NORMALS: [[f32; 3]; 6] = [
    [0.0, 1.0, 0.0],  // Top
    [0.0, 0.0, -1.0], // Back
    [-1.0, 0.0, 0.0], // Left
    [1.0, 0.0, 0.0],  // Right
    [0.0, 0.0, 1.0],  // Front
    [0.0, -1.0, 0.0], // Bottom
];

const CROSS_VERTICES: [[[f32; 3]; 4]; 2] = [
    [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ],
    [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
    ],
];

const CROSS_NORMALS: [[f32; 3]; 2] = [[1.0, 0.0, -1.0], [-1.0, 0.0, -1.0]];

// TODO: Idk if it makes sense to have this here. Might makes sense to move the load_blocks
// function over to the assets, but keep the Blocks struct here, as it is where you would expect to
// find it.
pub fn load_blocks(
    asset_server: Res<AssetServer>,
    net: Res<NetworkClient>,
    server_config: Res<messages::ServerConfig>,
    block_textures: Res<assets::BlockTextures>,
    material_handles: Res<assets::Materials>,
    materials: Res<Assets<BlockMaterial>>,
) {
    if server_config.block_ids.len() > u16::MAX as usize {
        net.disconnect(&format!(
            "Misconfigured assets: too many blocks, {} is the limit, but {} were supplied.",
            BlockId::MAX,
            server_config.block_ids.len()
        ));
        return;
    }

    let mut block_ids = server_config.block_ids.clone();
    let mut maybe_blocks = Vec::new();
    maybe_blocks.resize_with(block_ids.len(), Option::default);

    // Recursively walk block configuration directory
    fn walk_dir<T: AsRef<std::path::Path>>(
        dir: T,
    ) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut files = Vec::new();

        let directory = std::fs::read_dir(dir)?;

        for entry in directory {
            let file_path = entry?.path();

            if file_path.is_dir() {
                let sub_files = walk_dir(&file_path)?;
                files.extend(sub_files);
            } else {
                files.push(file_path);
            }
        }

        Ok(files)
    }

    let files = match walk_dir(BLOCK_CONFIG_PATH) {
        Ok(f) => f,
        Err(e) => {
            net.disconnect(&format!(
                "Failed to read file paths from the block configuration directory.\nError: {}",
                e
            ));
            return;
        }
    };

    for file_path in files {
        let block_config_json = match BlockConfig::read_as_json(&file_path) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: failed to read block config at {}\nError: {}",
                    file_path.display(),
                    e
                ));
                return;
            }
        };

        let block_config = if block_config_json.get("name").is_some() {
            match serde_json::from_value(block_config_json) {
                Ok(result) => result,
                Err(e) => {
                    net.disconnect(&format!(
                        "Misconfigured assets: failed to read block config at {}\nError: {}",
                        file_path.display(),
                        e
                    ));
                    return;
                }
            }
        } else {
            // Blocks without names are parents and are therefore ignored
            continue;
        };

        let block = match block_config {
            BlockConfig::Cube {
                name,
                faces,
                quads,
                friction,
                drag,
                material,
                only_cull_self,
                interactable,
                light_attenuation,
                light,
                fog,
                sound,
                placement,
            } => {
                let material_handle = if let Some(m) = material_handles.get(&material) {
                    m.clone().typed()
                } else {
                    net.disconnect(&format!(
                        "Misconfigured assets: tried to use material '{}' for block '{}', \
                        but the material does not exist.",
                        material, name
                    ));
                    return;
                };
                let material = materials.get(&material_handle).unwrap();

                let mut mesh_primitives = Vec::new();

                if let Some(faces) = faces {
                    for (i, face_name) in [
                        &faces.top,
                        &faces.front,
                        &faces.left,
                        &faces.right,
                        &faces.back,
                        &faces.bottom,
                    ]
                    .iter()
                    .enumerate()
                    {
                        let texture_array_id = match block_textures.get(face_name) {
                            Some(id) => *id,
                            None => {
                                net.disconnect(format!(
                                    "Misconfigured assets: failed to read block at: {}, no block texture with the name {}",
                                    file_path.display(),
                                    face_name
                                ));
                                return;
                            }
                        };

                        let face = match i {
                            0 => BlockFace::Top,
                            1 => BlockFace::Back,
                            2 => BlockFace::Left,
                            3 => BlockFace::Right,
                            4 => BlockFace::Front,
                            5 => BlockFace::Bottom,
                            _ => unreachable!(),
                        };

                        let square = QuadPrimitive {
                            vertices: FACE_VERTICES[i],
                            normals: [FACE_NORMALS[i], FACE_NORMALS[i]],
                            texture_array_id,
                            cull_face: Some(face),
                            light_face: face,
                            rotate_texture: false,
                        };

                        mesh_primitives.push(square);
                    }
                }

                let mut cull_delimiters = [None, None, None, None];

                if let Some(quads) = quads {
                    for quad in quads.iter() {
                        let texture_array_id = match block_textures.get(&quad.texture) {
                            Some(id) => *id,
                            None => {
                                net.disconnect(format!(
                                    "Misconfigured assets: failed to read block at: {}, no block texture with the name {}",
                                    file_path.display(),
                                    &quad.texture
                                ));
                                return;
                            }
                        };

                        let normals = [
                            (Vec3::from_array(quad.vertices[1])
                                - Vec3::from_array(quad.vertices[0]))
                            .cross(
                                Vec3::from_array(quad.vertices[2])
                                    - Vec3::from_array(quad.vertices[1]),
                            )
                            .to_array(),
                            (Vec3::from_array(quad.vertices[3])
                                - Vec3::from_array(quad.vertices[1]))
                            .cross(
                                Vec3::from_array(quad.vertices[2])
                                    - Vec3::from_array(quad.vertices[1]),
                            )
                            .to_array(),
                        ];

                        let normal = Vec3::from(normals[0]);
                        let normal_max =
                            normal.abs().cmpeq(Vec3::splat(normal.abs().max_element()));
                        let light_face = if normal_max.x {
                            if normal.x.is_sign_positive() {
                                BlockFace::Right
                            } else {
                                BlockFace::Left
                            }
                        } else if normal_max.y {
                            if normal.y.is_sign_positive() {
                                BlockFace::Top
                            } else {
                                BlockFace::Bottom
                            }
                        } else if normal_max.z {
                            if normal.z.is_sign_positive() {
                                BlockFace::Front
                            } else {
                                BlockFace::Back
                            }
                        } else {
                            unreachable!();
                        };

                        match quad.cull_face {
                            Some(BlockFace::Top) | Some(BlockFace::Bottom) => (),
                            Some(b) => {
                                if quad.vertices[0][1] != 1.0 || quad.vertices[2][1] != 1.0 {
                                    // Top left -> top right and vice versa to mirror it to how a
                                    // facing block would see it.
                                    cull_delimiters[b as usize] =
                                        Some((quad.vertices[2][1], quad.vertices[0][1]));
                                }
                            }
                            None => (),
                        }

                        mesh_primitives.push(QuadPrimitive {
                            vertices: quad.vertices,
                            normals,
                            texture_array_id,
                            cull_face: quad.cull_face,
                            light_face,
                            rotate_texture: quad.rotate_texture,
                        });
                    }
                }

                let cull_method = if only_cull_self {
                    CullMethod::OnlySelf
                } else {
                    match material.alpha_mode {
                        AlphaMode::Opaque => CullMethod::All,
                        AlphaMode::Mask(_) => CullMethod::None,
                        _ => CullMethod::TransparentOnly,
                    }
                };

                let fog_settings = if let Some(fog) = fog {
                    Some(DistanceFog {
                        color: fog.color,
                        falloff: FogFalloff::Linear {
                            start: fog.start,
                            end: fog.stop,
                        },
                        ..default()
                    })
                } else {
                    None
                };

                Block::Cube(Cube {
                    name,
                    material_handle,
                    quads: mesh_primitives,
                    friction,
                    drag,
                    interactable,
                    cull_method,
                    cull_delimiters,
                    light_attenuation: light_attenuation.unwrap_or(15).min(15),
                    light: light.min(15),
                    fog_settings,
                    sound,
                    placement,
                })
            }

            BlockConfig::Model {
                name,
                model,
                friction,
                drag,
                interactable,
                sound,
                light,
                placement,
            } => {
                // TODO: model must cause a disconnect if not found
                let model = {
                    let path = MODEL_PATH.to_owned() + &model + ".glb#Scene0";
                    asset_server.load(&path)
                };

                Block::Model(BlockModel {
                    name,
                    model,
                    friction,
                    drag,
                    interactable,
                    sound,
                    light: light.min(15),
                    placement,
                })
            }
        };

        if let Some(block_id) = block_ids.remove(block.name()) {
            maybe_blocks[block_id as usize] = Some(block);
        }
    }

    if block_ids.len() > 0 {
        net.disconnect(format!(
            "Misconfigured assets: missing blocks: {:?}",
            block_ids.keys().collect::<Vec<_>>()
        ));
    }

    unsafe {
        BLOCKS.take();
        BLOCKS
            .set(Blocks {
                blocks: maybe_blocks.into_iter().flatten().collect(),
                block_ids: server_config.block_ids.clone(),
            })
            .unwrap();
    }
}

// TODO: Wrap into Blocks(_Blocks)? This way it can have 2 get functions. One for the OnceCell and
// one for getting blocks. Just implement deref for blocks. [index] for blocks looks really
// awkward.
/// The configurations for all the blocks.
#[derive(Debug, Default)]
pub struct Blocks {
    blocks: Vec<Block>,
    // Map from block name to block id
    block_ids: HashMap<String, BlockId>,
}

impl Blocks {
    #[track_caller]
    pub fn get() -> &'static Self {
        unsafe {
            return BLOCKS
                .get()
                .expect("The blocks have not been loaded yet, make sure this is only used after they have been.");
        }
    }

    pub fn get_config(&self, id: BlockId) -> &Block {
        return &self.blocks[id as usize];
    }

    pub fn get_id(&self, name: &str) -> Option<&BlockId> {
        return self.block_ids.get(name);
    }

    pub fn contains(&self, block_id: BlockId) -> bool {
        return (block_id as usize) < self.blocks.len();
    }
}

#[derive(Debug)]
pub struct Cube {
    // Name of the block
    name: String,
    // Material used to render this block.
    pub material_handle: Handle<materials::BlockMaterial>,
    // List of squares meshes that make up the block.
    pub quads: Vec<QuadPrimitive>,
    // The friction of the block's surfaces.
    friction: Option<Friction>,
    // The drag when inside the block
    drag: Vec3,
    // If when the player uses their equipped item on this block it should count as an
    // interaction(true), or if it should count as trying to place its associated block(false).
    interactable: bool,
    // The alpha mode of the blocks associated material, used to determine face culling.
    cull_method: CullMethod,
    // TODO: This is not strictly needed I think, and it makes the code messy in a direction I
    // don't like. It was needed for water, but I don't think it's actually needed for anything
    // else. Water could be implemented by having many more water blocks to ensure that all
    // possible water states are covered. This would also make water feel fluent, which it doesn't
    // currently. I didn't because I feel bad about wasting some 500 blocks (9!/6! I think is
    // correct.)
    //
    // Two vertical points on the left and right side of the vertical block faces that make a line
    // delimiting how much of an adjacent block face it will cull. This is needed for transparent
    // blocks like water as you only want the parts exposed to air to render when two water blocks
    // of different levels are adjacent to each other.
    cull_delimiters: [Option<(f32, f32)>; 4],
    // How much the block attenuates light. '0' will make sunlight travel downwards unimpeded, but
    // otherwise as if '1'.
    light_attenuation: u8,
    // Fog rendered if the camera is inside the bounds of the cube.
    pub fog_settings: Option<DistanceFog>,
    // Sounds played when walked on or in (random pick)
    sound: Sound,
    // Light emitted by the block
    light: u8,
    // How the block can be placed
    placement: BlockPlacement,
}

// TODO: This was made before the Models collection was made. This could hold model ids instead of
// the handles. I have hardcoded the glb extension here, which would no longer be a thing.
//
// Models are used to render all blocks of irregular shape. Even though they are assigned block
// ids, they are not used in interaction with the server
#[derive(Debug)]
pub struct BlockModel {
    // Name of the block
    name: String,
    /// Model used when centered in the block
    pub model: Handle<Scene>,
    // The friction of the block's surfaces, applied by closest normal of the textures.
    friction: Option<Friction>,
    // The drag when inside the block
    drag: Vec3,
    // If when the player uses their equipped item on this block, it should count as an
    // interaction, or it should count as trying to place a block.
    interactable: bool,
    // Sounds played when walked on or in (random pick)
    sound: Sound,
    // Light emitted by the block
    light: u8,
    // How the block can be placed
    placement: BlockPlacement,
}

#[derive(Debug)]
pub enum Block {
    Cube(Cube),
    Model(BlockModel),
}

impl Block {
    pub fn cull_delimiter(&self, block_face: BlockFace) -> Option<(f32, f32)> {
        match self {
            Block::Cube(cube) => match block_face {
                BlockFace::Top | BlockFace::Bottom => None,
                b => cube.cull_delimiters[b as usize],
            },
            Block::Model(_) => None,
        }
    }

    pub fn culls(&self, other: &Block) -> bool {
        match self {
            Block::Cube(cube) => {
                let Block::Cube(other_cube) = other else {
                    unreachable!()
                };
                match cube.cull_method {
                    CullMethod::All => true,
                    CullMethod::None => false,
                    CullMethod::TransparentOnly => {
                        other_cube.cull_method == CullMethod::TransparentOnly
                    }
                    // TODO: This isn't correct on purpose, the blocks should be compared. Could be by id,
                    // but I don't have that here. Comparing by name is expensive, don't want to.
                    // Will fuck up if two different blocks are put together. Can use const* to
                    // compare pointer?
                    CullMethod::OnlySelf => other_cube.cull_method == CullMethod::OnlySelf,
                }
            }
            Block::Model(_) => false,
        }
    }

    pub fn is_transparent(&self) -> bool {
        match self {
            Block::Cube(c) => match c.cull_method {
                CullMethod::All => false,
                _ => true,
            },
            Block::Model(_) => true,
        }
    }

    pub fn can_have_block_state(&self) -> bool {
        match self {
            Block::Cube(cube) => {
                cube.placement.rotatable || cube.placement.side_transform.is_some()
            }
            // Block models aren't handled by the client, but sent as separate models by the
            // server.
            Block::Model(_model) => false,
        }
    }

    pub fn friction(&self) -> Option<&Friction> {
        match self {
            Block::Cube(cube) => cube.friction.as_ref(),
            Block::Model(model) => model.friction.as_ref(),
        }
    }

    pub fn drag(&self) -> Vec3 {
        match self {
            Block::Cube(cube) => cube.drag,
            Block::Model(model) => model.drag,
        }
    }

    pub fn light_attenuation(&self) -> u8 {
        match self {
            Block::Cube(c) => c.light_attenuation,
            Block::Model(_) => 0,
        }
    }

    pub fn light_level(&self) -> u8 {
        match self {
            Block::Cube(c) => c.light,
            Block::Model(m) => m.light,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Block::Cube(c) => &c.name,
            Block::Model(m) => &m.name,
        }
    }

    pub fn fog_settings(&self) -> Option<DistanceFog> {
        match self {
            Block::Cube(c) => c.fog_settings.clone(),
            Block::Model(_) => None,
        }
    }

    pub fn step_sounds(&self) -> &Vec<String> {
        // Random index, don't know if correct
        match self {
            Block::Cube(c) => &c.sound.step,
            Block::Model(m) => &m.sound.step,
        }
    }
}

// bits:
//     0000 0000 0000 unused
//     0000
//       ^^-north/south/east/west
//      ^---centered, overrides previous rotation, 1 = centered
//     ^----upside down
#[derive(Debug, Clone, Copy)]
pub struct BlockState(pub u16);

impl Default for BlockState {
    fn default() -> Self {
        Self(0b100)
    }
}

impl BlockState {
    pub fn new(rotation: BlockRotation) -> Self {
        return Self(rotation as u16);
    }

    pub fn rotation(&self) -> BlockRotation {
        if self.0 & 0b100 == 0 {
            return unsafe { std::mem::transmute(self.0 & 0b11) };
        } else {
            return BlockRotation::None;
        }
    }

    pub fn uses_side_model(&self) -> bool {
        return self.0 & 0b100 == 0;
    }

    pub fn is_upside_down(&self) -> bool {
        return self.0 & 0b1000 != 0;
    }
}

// Clockwise rotation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockRotation {
    None = 0,
    Once,
    Twice,
    Thrice,
}

impl BlockRotation {
    // Bevy's coordinate system has +z out of the screen, +y up, +x right so if you do a
    // normal rotation it would look like it's moving clockwise when viewing it from above. Since
    // rotations are expected to be counter clockwise the rotation is inverted. Making the true
    // rotation clockwise, but when you view it from above it will look counter clockwise. This is
    // to make it easier to rotate mentally.
    pub fn rotate_vertex(&self, vertex: &mut [f32; 3]) {
        let cos = (*self as u16 as f32 * std::f32::consts::FRAC_PI_2).cos();
        let sin = (*self as u16 as f32 * std::f32::consts::FRAC_PI_2).sin();
        let new_x = 0.5 + cos * (vertex[0] - 0.5) + sin * (vertex[2] - 0.5);
        let new_z = 0.5 - sin * (vertex[0] - 0.5) + cos * (vertex[2] - 0.5);
        vertex[0] = new_x;
        vertex[2] = new_z;
    }

    pub fn as_quat(&self) -> Quat {
        match self {
            Self::None => Quat::from_rotation_y(0.0),
            Self::Once => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Self::Twice => Quat::from_rotation_y(std::f32::consts::PI),
            Self::Thrice => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        }
    }
}

/// Block config that is stored on file.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum BlockConfig {
    // There is easy way to define a cube, and hard. Give 'faces' and it will generate cube mesh on
    // its own. Give quads and the cube can take on non-cube shapes.
    Cube {
        /// Name of the block, must be unique
        name: String,
        // TODO: Rename both field name and struct name.
        //
        /// Convenient way to define a block as opposed to having to define it through the quads.
        faces: Option<CubeMeshTextureNames>,
        /// List of quads that make up a mesh.
        quads: Option<Vec<QuadPrimitiveJson>>,
        /// The friction of the block's surfaces.
        friction: Option<Friction>,
        // The drag when inside the block
        #[serde(default)]
        drag: Vec3,
        /// Material that should be used to render the block.
        material: String,
        /// If the block should only cull quads from blocks of the same type.
        #[serde(default)]
        only_cull_self: bool,
        /// Marking a block as interactable makes it so clicking it
        #[serde(default)]
        interactable: bool,
        /// How many levels light should decrease when passing through this block.
        light_attenuation: Option<u8>,
        /// Light emitted by the block
        #[serde(default)]
        light: u8,
        /// If fog should be rendered when the player camera is inside the block.
        fog: Option<FogJson>,
        /// Sounds played when walking on/in block
        #[serde(default)]
        sound: Sound,
        /// Block placement rules
        #[serde(default)]
        placement: BlockPlacement,
    },
    Model {
        /// Name of the block, must be unique
        name: String,
        /// Name of the model file
        model: String,
        /// The friction of the block's surfaces.
        friction: Option<Friction>,
        // The drag when inside the block
        #[serde(default)]
        drag: Vec3,
        /// If the block is interactable
        #[serde(default)]
        interactable: bool,
        /// Sounds played when walking on/in block
        #[serde(default)]
        sound: Sound,
        /// Light emitted by the block
        #[serde(default)]
        light: u8,
        /// Block placement rules
        #[serde(default)]
        placement: BlockPlacement,
    },
}

impl BlockConfig {
    fn read_as_json(
        path: &std::path::Path,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(&path)?;

        let mut config: serde_json::Value = serde_json::from_reader(&file)?;

        // recursively read parent configs
        if let Some(parent) = config["parent"].as_str() {
            let parent_path = std::path::Path::new(BLOCK_CONFIG_PATH).join(parent);
            let mut parent: serde_json::Value = match Self::read_as_json(&parent_path) {
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

            // Append to parent to replace the values it shares with the child.
            parent
                .as_object_mut()
                .unwrap()
                .append(&mut config.as_object_mut().unwrap());

            return Ok(parent);
        }

        return Ok(config);
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Sound {
    #[serde(default)]
    pub step: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct BlockPlacement {
    // Set if the block can be placed by clicking the top face of the block below
    floor: bool,
    // Set if the block can be placed by clicking the bottom face of the block above
    ceiling: bool,
    // Set if the block can be placed by clicking the sides of adjacent blocks
    sides: bool,
    // Set if the block should always be rotated when placed.
    rotatable: bool,
    // Set if a transform should be applied when placing on a sideways adjacent block. This will
    // rotate the block even if 'rotatable' is not set, but only on sides.
    side_transform: Option<Transform>,
}

impl Default for BlockPlacement {
    fn default() -> Self {
        Self {
            floor: true,
            ceiling: true,
            sides: true,
            rotatable: false,
            side_transform: None,
        }
    }
}

// This is derived from the AlphaMode of the block's material as well as the BlockConfig::Cube
// attribute 'only_cull_self'.
// only_cull_self==true -> OnlySelf e.g. glass, this takes presedence over AlphaMode
// AlphaMode::Opaque -> All e.g. stone
// All blending AlphaMode's -> TransparentOnly e.g. water
// AlphaMode::Mask -> None e.g. leaves
#[derive(Debug, PartialEq)]
enum CullMethod {
    // Cull all faces that are adjacent to the block.
    All,
    // Cull only other transparent faces that are adjacent.
    TransparentOnly,
    // Do not cull.
    None,
    // Only cull adjacent faces when the block is of the same type.
    OnlySelf,
}

#[derive(Debug)]
pub struct QuadPrimitive {
    /// Vertices of the 4 corners of the square.
    pub vertices: [[f32; 3]; 4],
    /// Normals for both triangles.
    pub normals: [[f32; 3]; 2],
    /// Index id in the texture array.
    pub texture_array_id: u32,
    /// Which adjacent block face culls this quad from rendering.
    pub cull_face: Option<BlockFace>,
    /// Which blockface this quad will take it's lighting from.
    pub light_face: BlockFace,
    pub rotate_texture: bool,
}

#[derive(Deserialize)]
struct QuadPrimitiveJson {
    // indexing
    // 1   3
    // | \ |
    // 0   2
    vertices: [[f32; 3]; 4],
    texture: String,
    cull_face: Option<BlockFace>,
    #[serde(default)]
    rotate_texture: bool,
}

#[derive(Deserialize)]
struct FogJson {
    color: Color,
    start: f32,
    stop: f32,
}

#[derive(Deserialize)]
struct CubeMeshTextureNames {
    top: String,
    bottom: String,
    left: String,
    right: String,
    front: String,
    back: String,
}

// The different faces of a block
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockFace {
    // +X direction
    Right,
    Left,
    // +Z direction
    Front,
    Back,
    // +Y direction
    Top,
    Bottom,
}

impl BlockFace {
    pub fn to_rotation(&self) -> BlockRotation {
        match self {
            Self::Front => BlockRotation::None,
            Self::Right => BlockRotation::Once,
            Self::Back => BlockRotation::Twice,
            Self::Left => BlockRotation::Thrice,
            _ => unreachable!(),
        }
    }
    pub fn rotate(&self, rotation: BlockRotation) -> BlockFace {
        match self {
            BlockFace::Right => match rotation {
                BlockRotation::None => BlockFace::Right,
                BlockRotation::Once => BlockFace::Back,
                BlockRotation::Twice => BlockFace::Left,
                BlockRotation::Thrice => BlockFace::Front,
            },
            BlockFace::Front => match rotation {
                BlockRotation::None => BlockFace::Front,
                BlockRotation::Once => BlockFace::Right,
                BlockRotation::Twice => BlockFace::Back,
                BlockRotation::Thrice => BlockFace::Left,
            },
            BlockFace::Left => match rotation {
                BlockRotation::None => BlockFace::Left,
                BlockRotation::Once => BlockFace::Front,
                BlockRotation::Twice => BlockFace::Right,
                BlockRotation::Thrice => BlockFace::Back,
            },
            BlockFace::Back => match rotation {
                BlockRotation::None => BlockFace::Back,
                BlockRotation::Once => BlockFace::Left,
                BlockRotation::Twice => BlockFace::Front,
                BlockRotation::Thrice => BlockFace::Right,
            },
            BlockFace::Top => BlockFace::Top,
            BlockFace::Bottom => BlockFace::Bottom,
        }
    }

    pub fn reverse_rotate(&self, rotation: BlockRotation) -> BlockFace {
        return self.rotate(match rotation {
            BlockRotation::None => BlockRotation::None,
            BlockRotation::Once => BlockRotation::Thrice,
            BlockRotation::Twice => BlockRotation::Twice,
            BlockRotation::Thrice => BlockRotation::Once,
        });
    }

    pub fn opposite(&self) -> Self {
        match self {
            BlockFace::Right => BlockFace::Left,
            BlockFace::Left => BlockFace::Right,
            BlockFace::Front => BlockFace::Back,
            BlockFace::Back => BlockFace::Front,
            BlockFace::Top => BlockFace::Bottom,
            BlockFace::Bottom => BlockFace::Top,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Friction {
    pub front: f32,
    pub back: f32,
    pub right: f32,
    pub left: f32,
    pub top: f32,
    pub bottom: f32,
}
