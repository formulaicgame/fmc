use std::{collections::HashMap, path::PathBuf};

use bevy::{prelude::*, render::primitives::Aabb};
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

const FACE_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

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
    material_handles: Res<assets::Materials<BlockMaterial>>,
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
        let block_config_json = match BlockJson::read_as_json(&file_path) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: Failed to read block config at {}\nError: {}",
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
                        "Misconfigured assets: Failed to read block config at {}\nError: {}",
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
            BlockJson::Cube {
                name,
                faces,
                quads,
                friction,
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
                    m.clone()
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
                        &faces.back,
                        &faces.left,
                        &faces.right,
                        &faces.front,
                        &faces.bottom,
                    ]
                    .iter()
                    .enumerate()
                    {
                        let texture_array_id = match block_textures.get(face_name) {
                            Some(id) => *id,
                            None => {
                                net.disconnect(format!(
                                    "Misconfigured assets: Failed to read block at: {}, no block texture with the name {}",
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
                            uvs: FACE_UVS.clone(),
                            texture_array_id,
                            cull_face: Some(face),
                            light_face: Some(face),
                            rotate_texture: false,
                        };

                        mesh_primitives.push(square);
                    }
                }

                let cull_method = if only_cull_self {
                    CullMethod::OnlySelf
                } else {
                    match material.alpha_mode {
                        AlphaMode::Opaque => {
                            if quads.is_some() {
                                // TODO: This is because of things like stairs that have open
                                // sections. It should be possible to define which faces a block
                                // culls. It also uses this to determine transparency currently.
                                //
                                // Any block that is opaque but has custom quads does not cull
                                // anything.
                                CullMethod::None
                            } else {
                                CullMethod::All
                            }
                        }
                        AlphaMode::Mask(_) => CullMethod::None,
                        _ => CullMethod::TransparentOnly,
                    }
                };

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

                        let mut uvs: [[f32; 2]; 4] = default();
                        for (i, vertex) in quad.vertices.into_iter().enumerate() {
                            let uv = if normal_max.x {
                                Vec3::from_array(vertex).zy()
                            } else if normal_max.y {
                                Vec3::from_array(vertex).xz()
                            } else {
                                Vec3::from_array(vertex).xy()
                            };

                            if i == 0 {
                                uvs[0] = [uv.x - uv.x.floor(), uv.y - uv.y.floor()];
                            } else if i == 1 {
                                uvs[1] = [
                                    // This is just the fraction, but instead of using `f32::fract`
                                    // we do this so it's inversed for negative numbers. e.g. -0.6
                                    // yields 0.4, which is what we want because that is the distance
                                    // from -1.0 to -0.6
                                    uv.x - uv.x.floor(),
                                    // Since this is on the high side of the range extracting the fract is harder since it
                                    // can be a whole number. e.g a position of 1.0 should give a fraction of 1.0, not 0.0.
                                    uv.y - (uv.y.ceil() - 1.0),
                                ];
                            } else if i == 2 {
                                uvs[2] = [uv.x - (uv.x.ceil() - 1.0), uv.y - uv.y.floor()];
                            } else if i == 3 {
                                uvs[3] = [uv.x - (uv.x.ceil() - 1.0), uv.y - (uv.y.ceil() - 1.0)];
                            }
                        }

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
                            uvs,
                            texture_array_id,
                            cull_face: quad.cull_face,
                            light_face: quad.cull_face,
                            rotate_texture: quad.rotate_texture,
                        });
                    }
                }

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

                let aabb = Aabb::enclosing(
                    mesh_primitives
                        .iter()
                        .map(|p| p.vertices)
                        .flatten()
                        .map(|vertex| Vec3::from_slice(&vertex)),
                )
                .unwrap_or(Aabb::from_min_max(Vec3::ZERO, Vec3::ONE));

                Block {
                    name,
                    model: None,
                    material_handle,
                    quads: mesh_primitives,
                    friction,
                    interactable,
                    cull_method,
                    cull_delimiters,
                    light: light.min(15),
                    light_attenuation: light_attenuation.unwrap_or(15).min(15),
                    fog_settings,
                    sound,
                    placement,
                    aabb,
                }
            }

            BlockJson::Model {
                name,
                model,
                friction,
                interactable,
                sound,
                light,
                placement,
            } => {
                // TODO: model must cause a disconnect if not found
                let model = {
                    let path = MODEL_PATH.to_owned() + &model + ".glb#Scene0";
                    Some(asset_server.load(&path))
                };

                Block {
                    name,
                    model,
                    material_handle: Handle::default(),
                    quads: Vec::new(),
                    friction,
                    interactable,
                    cull_method: CullMethod::None,
                    cull_delimiters: Default::default(),
                    light: light.min(15),
                    light_attenuation: 1,
                    fog_settings: None,
                    sound,
                    placement,
                    aabb: Aabb::from_min_max(Vec3::ZERO, Vec3::ONE),
                }
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
pub struct Block {
    // Name of the block
    name: String,
    // The model used to render the block. This is unused, block models are spawned as regular
    // models on the server. If a block that uses a model is in a chunk, it will just be ignored.
    model: Option<Handle<Scene>>,
    // Material used to render this block.
    pub material_handle: Handle<materials::BlockMaterial>,
    // List of squares meshes that make up the block.
    pub quads: Vec<QuadPrimitive>,
    // The surface friction or drag
    friction: Friction,
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
    // Light emitted by the block
    light: u8,
    // How much the block attenuates light. '0' will make sunlight travel downwards unimpeded, but
    // otherwise as if '1'.
    light_attenuation: u8,
    // Fog rendered if the camera is inside the bounds of the cube.
    fog_settings: Option<DistanceFog>,
    // Sounds played when walked on or in (random pick)
    sound: Sound,
    // How the block can be placed
    placement: BlockPlacement,
    // The bounding box of the block
    aabb: Aabb,
}

impl Block {
    pub fn is_block_model(&self) -> bool {
        self.model.is_some()
    }

    pub fn is_solid(&self) -> bool {
        matches!(self.friction, Friction::Surface { .. })
    }

    pub fn cull_delimiter(&self, block_face: BlockFace) -> Option<(f32, f32)> {
        match block_face {
            BlockFace::Top | BlockFace::Bottom => None,
            b => self.cull_delimiters[b as usize],
        }
    }

    pub fn culls(&self, other: &Block) -> bool {
        match self.cull_method {
            CullMethod::All => true,
            CullMethod::None => false,
            CullMethod::TransparentOnly => other.cull_method == CullMethod::TransparentOnly,
            // TODO: This isn't correct on purpose, the blocks should be compared. Could be by id,
            // but I don't have that here. Comparing by name is expensive, don't want to.
            // Will fuck up if two different blocks are put together. Can use const* to
            // compare pointer?
            CullMethod::OnlySelf => other.cull_method == CullMethod::OnlySelf,
        }
    }

    pub fn can_have_block_state(&self) -> bool {
        self.placement.rotatable || self.placement.side_transform.is_some()
    }

    pub fn surface_friction(&self, block_face: BlockFace) -> Vec3 {
        let friction = match self.friction {
            Friction::Surface {
                front,
                back,
                right,
                left,
                top,
                bottom,
            } => match block_face {
                BlockFace::Front => front,
                BlockFace::Back => back,
                BlockFace::Right => right,
                BlockFace::Left => left,
                BlockFace::Top => top,
                BlockFace::Bottom => bottom,
            },
            Friction::Drag(_) => 0.0,
        };

        Vec3::splat(friction)
    }

    pub fn drag(&self) -> Vec3 {
        match self.friction {
            Friction::Drag(drag) => drag,
            Friction::Surface { .. } => Vec3::ZERO,
        }
    }

    pub fn light_attenuation(&self) -> u8 {
        self.light_attenuation
    }

    pub fn light_level(&self) -> u8 {
        self.light
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fog_settings(&self) -> Option<&DistanceFog> {
        self.fog_settings.as_ref()
    }

    pub fn step_sounds(&self) -> &Vec<String> {
        &self.sound.step
    }

    pub fn aabb(&self) -> &Aabb {
        &self.aabb
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
enum BlockJson {
    Cube {
        /// Name of the block, must be unique
        name: String,
        // TODO: Rename both field name and struct name.
        //
        /// Convenient way to define a block as opposed to having to define it through the quads.
        faces: Option<CubeMeshTextureNames>,
        /// List of quads that make up a mesh.
        quads: Option<Vec<QuadPrimitiveJson>>,
        /// The friction of the block's surfaces or the drag.
        #[serde(default)]
        friction: Friction,
        /// Material that should be used to render the block.
        material: String,
        /// This block will only cull the faces of other blocks when they are of the same type.
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
        /// The friction of the block's surfaces or the drag.
        #[serde(default)]
        friction: Friction,
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

impl BlockJson {
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
    //floor: bool,
    // Set if the block can be placed by clicking the bottom face of the block above
    //ceiling: bool,
    // Set if the block can be placed by clicking the sides of adjacent blocks
    //sides: bool,
    // Set if the block should always be rotated when placed.
    #[serde(default)]
    rotatable: bool,
    // Set if a transform should be applied when placing on a sideways adjacent block. This will
    // rotate the block even if 'rotatable' is not set, but only on sides.
    side_transform: Option<Transform>,
}

impl Default for BlockPlacement {
    fn default() -> Self {
        Self {
            // floor: true,
            // ceiling: true,
            // sides: true,
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
    /// Uv coordinates for all 4 corners
    pub uvs: [[f32; 2]; 4],
    /// Index id in the texture array.
    pub texture_array_id: u32,
    /// Which adjacent block face culls this quad from rendering.
    pub cull_face: Option<BlockFace>,
    /// Which blockface this quad will be lit as. If None it will use the block's own light value.
    pub light_face: Option<BlockFace>,
    pub rotate_texture: bool,
}

#[derive(Deserialize)]
struct QuadPrimitiveJson {
    // indexing
    // 0   2
    // | / |
    // 1   3
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
#[serde(untagged)]
pub enum Friction {
    Surface {
        front: f32,
        back: f32,
        right: f32,
        left: f32,
        top: f32,
        bottom: f32,
    },
    Drag(Vec3),
}

impl Default for Friction {
    fn default() -> Self {
        Self::Drag(Vec3::ZERO)
    }
}
