use std::{collections::HashMap, path::PathBuf};

use bevy::prelude::*;
use fmc_networking::{messages, NetworkClient};
use serde::Deserialize;

use crate::{
    assets,
    rendering::materials::{self, BlockMaterial},
};

pub type BlockId = u16;
pub static mut BLOCKS: std::sync::OnceLock<Blocks> = std::sync::OnceLock::new();

const MODEL_PATH: &str = "server_assets/textures/models/";

const BLOCK_CONFIG_PATH: &str = "server_assets/blocks/";

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
            "Misconfigured resource pack, too many blocks, {} is the limit, but {} were supplied.",
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
                    "Misconfigured resource pack, failed to read block config at {}\nError: {}",
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
                        "Misconfigured resource pack, failed to read block config at {}\nError: {}",
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
                material,
                only_cull_self,
                interactable,
                is_rotatable,
                light_attenuation,
                fog,
                sound,
            } => {
                let material_handle = if let Some(m) = material_handles.get(&material) {
                    m.clone().typed()
                } else {
                    net.disconnect(&format!(
                        "Misconfigured resource pack, tried to use material '{}' for block '{}', \
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
                                    "Misconfigured resource pack, failed to read block at: {}, no block texture with the name {}",
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
                                    "Misconfigured resource pack, failed to read block at: {}, no block texture with the name {}",
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
                    Some(FogSettings {
                        color: fog.color,
                        falloff: FogFalloff::from_visibility_squared(fog.distance),
                        //falloff: FogFalloff::Linear {
                        //    start: 0.0,
                        //    end: fog.distance,
                        //},
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
                    interactable,
                    cull_method,
                    cull_delimiters,
                    is_rotatable,
                    light_attenuation: light_attenuation.unwrap_or(15).min(15),
                    fog_settings,
                    sound,
                })
            }

            BlockConfig::Model {
                name,
                center_model,
                side_model,
                friction,
                interactable,
                sound,
            } => {
                let center_model = if let Some(center_model) = center_model {
                    let path = MODEL_PATH.to_owned() + &center_model.name + ".glb#Scene0";
                    Some((
                        asset_server.load::<Scene>(&path),
                        Transform {
                            translation: center_model.position,
                            rotation: center_model.rotation,
                            scale: Vec3::ONE,
                        },
                    ))
                } else {
                    None
                };

                let side_model = if let Some(side_model) = side_model {
                    let path = MODEL_PATH.to_owned() + &side_model.name + ".glb#Scene0";
                    Some((
                        asset_server.load::<Scene>(&path),
                        Transform {
                            translation: side_model.position,
                            rotation: side_model.rotation,
                            scale: Vec3::ONE,
                        },
                    ))
                } else {
                    None
                };

                if center_model.is_none() && side_model.is_none() {
                    net.disconnect(format!(
                        "Misconfigured resource pack, failed to read block at: {}, \
                        one of 'center_model' and 'side_model' must be defined",
                        file_path.display()
                    ));
                    return;
                }

                Block::Model(BlockModel {
                    name,
                    center: center_model,
                    side: side_model,
                    friction,
                    interactable,
                    sound,
                })
            }
        };

        if let Some(block_id) = block_ids.remove(block.name()) {
            maybe_blocks[block_id as usize] = Some(block);
        }
    }

    if block_ids.len() > 0 {
        net.disconnect(format!(
            "Misconfigured resource pack, missing blocks: {:?}",
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
    // Friction value for player contact.
    friction: Friction,
    // If when the player uses their equipped item on this block it should count as an
    // interaction, or it should count as trying to place its associated block.
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
    // If the block is rotateable around the y-axis
    is_rotatable: bool,
    // How much the block attenuates light. '0' will make sunlight travel downwards unimpeded, but
    // otherwise as if '1'.
    light_attenuation: u8,
    // Fog rendered if the camera is inside the bounds of the cube.
    pub fog_settings: Option<FogSettings>,
    // Sounds played when walked on or in (random pick)
    sound: Vec<String>,
}

// TODO: This was made before the Models collection was made. This could hold model ids instead of
// the handles. I have hardcoded the glb extension here, which would no longer be a thing.
//
// Models are used to render all blocks of irregular shape. There are multiple ways to place
// the model inside the cube. The server sends an orientation for all block models part of a
// chunk which define if it should be placed on the side of the block or in the center, if it
// should be upside down, and which direction it should point. Meanwhile when the player places
// a block, if the bottom surface is clicked it will place the center model(if defined) in the
// direction facing the player. If a side is clicked it will try to place the side model, if
// that is not available, it will fall back to the center model. One of them is always defined.
#[derive(Debug)]
pub struct BlockModel {
    /// Name of the block
    name: String,
    /// Model used when centered in the block
    pub center: Option<(Handle<Scene>, Transform)>,
    /// Model used when on the side of the block
    pub side: Option<(Handle<Scene>, Transform)>,
    /// Friction or drag, applied by closest normal of the textures.
    friction: Friction,
    /// If when the player uses their equipped item on this block, it should count as an
    /// interaction, or it should count as trying to place a block.
    interactable: bool,
    // Sounds played when walked on or in (random pick)
    sound: Vec<String>,
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
            Block::Cube(cube) => cube.is_rotatable,
            Block::Model(model) => model.side.is_some(),
        }
    }

    pub fn friction(&self) -> &Friction {
        match self {
            Block::Cube(cube) => &cube.friction,
            Block::Model(model) => &model.friction,
        }
    }

    pub fn light_attenuation(&self) -> u8 {
        match self {
            Block::Cube(c) => c.light_attenuation,
            Block::Model(_) => 1,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Block::Cube(c) => &c.name,
            Block::Model(m) => &m.name,
        }
    }

    pub fn fog_settings(&self) -> Option<FogSettings> {
        match self {
            Block::Cube(c) => c.fog_settings.clone(),
            Block::Model(_) => None,
        }
    }

    pub fn walking_sounds(&self) -> &Vec<String> {
        // Random index, don't know if correct
        match self {
            Block::Cube(c) => &c.sound,
            Block::Model(m) => &m.sound,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BlockState(pub u16);

impl BlockState {
    pub fn rotation(&self) -> BlockRotation {
        return unsafe { std::mem::transmute(self.0 & 0b11) };
    }

    pub fn uses_side_model(&self) -> bool {
        return self.0 & 0b100 != 0;
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
    // Bevy's coordinate system is so that +z is out of the screen, +y up, +x right so if you do a
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
        /// Convenient way to define a block as opposed to having to define it through the quads.
        faces: Option<TextureNames>,
        /// List of quads that make up a mesh.
        quads: Option<Vec<QuadPrimitiveJson>>,
        /// The friction or drag.
        friction: Friction,
        /// Material that should be used to render the block.
        material: String,
        /// If the block should only cull quads from blocks of the same type.
        #[serde(default)]
        only_cull_self: bool,
        /// If the block is interactable
        #[serde(default)]
        interactable: bool,
        /// If the block can rotate around the y axis
        #[serde(default)]
        is_rotatable: bool,
        /// How many levels light should decrease when passing through this block.
        light_attenuation: Option<u8>,
        /// If fog should be rendered when the player camera is inside the block.
        fog: Option<FogJson>,
        /// Sounds played when walking on/in block
        #[serde(default)]
        sound: Vec<String>,
    },
    Model {
        /// Name of the block, must be unique
        name: String,
        /// Name of model used when placed in the center of the block
        center_model: Option<ModelConfig>,
        /// Name of model used when placed on the side of the block
        side_model: Option<ModelConfig>,
        /// The friction or drag.
        friction: Friction,
        /// If the block is interactable
        #[serde(default)]
        interactable: bool,
        /// Sounds played when walking on/in block
        #[serde(default)]
        sound: Vec<String>,
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
    distance: f32,
}

#[derive(Deserialize)]
struct TextureNames {
    top: String,
    bottom: String,
    left: String,
    right: String,
    front: String,
    back: String,
}

#[derive(Deserialize)]
struct ModelConfig {
    name: String,
    #[serde(default)]
    position: Vec3,
    #[serde(default)]
    rotation: Quat,
}

// The different faces of a block
#[derive(Debug, Clone, Copy, Deserialize)]
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

    pub fn invert(&self) -> Self {
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
#[serde(rename_all = "lowercase")]
pub enum Friction {
    /// Friction for solid blocks.
    Static {
        front: f32,
        back: f32,
        right: f32,
        left: f32,
        top: f32,
        bottom: f32,
    },
    /// For non-collidable blocks, the friction is instead drag on the player movement.
    Drag(Vec3),
}
