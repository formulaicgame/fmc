use bevy::{
    gltf::{Gltf, GltfMesh, GltfPrimitive},
    prelude::*,
    render::{render_asset::RenderAssetUsages, render_resource::PrimitiveTopology},
    utils::HashMap,
};

use fmc_networking::{messages::ServerConfig, NetworkClient};
use serde::Deserialize;

const MODEL_PATH: &str = "server_assets/textures/models/";
const BLOCK_TEXTURE_PATH: &str = "server_assets/textures/blocks/";

pub type ModelId = u32;

pub(super) struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, register_gltf_animation_players);
    }
}

// TODO: Keeping all the models loaded will probably prove to be prohibitive. Need to be able to
// load them when needed, but still load them all fully once and validate so it can disconnect early.
//
/// A map from server model id to asset handle
#[derive(Resource)]
pub struct Models {
    id2model: std::collections::HashMap<ModelId, Model>,
    filename2id: std::collections::HashMap<String, ModelId>,
}

impl Models {
    pub fn get(&self, id: &u32) -> Option<&Model> {
        return self.id2model.get(&id);
    }

    pub fn get_id_by_filename(&self, filename: &str) -> Option<u32> {
        return self.filename2id.get(filename).cloned();
    }

    pub fn iter(&self) -> std::collections::hash_map::Values<u32, Model> {
        return self.id2model.values();
    }
}

#[derive(Component, Clone)]
pub struct Model {
    pub handle: Handle<Gltf>,
}

pub(super) fn load_models(
    mut commands: Commands,
    net: Res<NetworkClient>,
    server_config: Res<ServerConfig>,
    asset_server: Res<AssetServer>,
) {
    let mut models = Models {
        id2model: std::collections::HashMap::new(),
        filename2id: std::collections::HashMap::new(),
    };

    let directory = match std::fs::read_dir(MODEL_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(&format!(
                "Misconfigured resource pack: Failed to read model directory at '{}'\n Error: {}",
                MODEL_PATH, e
            ));
            return;
        }
    };

    // This is the genertic animation applied to runtime generated models. Models loaded from gltf
    // files should supply their own animation.
    let click_animation = asset_server.add(JsonModel::click_animation());

    let mut handles: HashMap<String, Handle<Gltf>> = HashMap::new();
    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured resource pack: Failed to read the file path of a model\n\
                    Error: {}",
                    e
                ));
                return;
            }
        };

        let model_name = path.file_stem().unwrap().to_string_lossy().into_owned();

        let Some(extension) = path.extension() else {
            net.disconnect(&format!(
                "Invalid model file at '{}', the file is missing its extension. \
                    Should be one of 'json', 'gltf' or 'glb'.",
                path.display()
            ));
            return;
        };

        let model_handle = if extension == "json" {
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
            let json_model: JsonModel = match serde_json::from_reader(file) {
                Ok(m) => m,
                Err(e) => {
                    net.disconnect(&format!(
                        "Misconfigured resource pack: Could not parse model at '{}'\nError: {e}",
                        path.display()
                    ));
                    return;
                }
            };
            let mut gltf = json_model.build_gltf(asset_server.as_ref());
            gltf.animations.push(click_animation.clone());
            gltf.named_animations
                .insert("left_click".to_owned(), click_animation.clone());
            asset_server.add(gltf)
        } else if extension == "glb" || extension == "gltf" {
            asset_server.load(path)
        } else {
            //net.disconnect(message);
            panic!();
            return;
        };

        handles.insert(model_name, model_handle);
    }

    for (name, id) in server_config.model_ids.iter() {
        if let Some(handle) = handles.remove(name) {
            models.filename2id.insert(name.to_owned(), *id);
            models.id2model.insert(*id, Model { handle });
        } else {
            net.disconnect(&format!(
                "Misconfigured resource pack: Missing model, no model with the name '{}'",
                name
            ));
        }
    }

    commands.insert_resource(models);
}

const BLOCK_MODEL_VERTICES: [[[f32; 3]; 6]; 6] = [
    // Top
    [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
    ],
    // Bottom
    [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ],
    // Left
    [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
    ],
    // Right
    [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
    ],
    // Front
    [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ],
    // Back
    [
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ],
];

const BLOCK_MODEL_UVS: [[f32; 2]; 6] = [
    [0.0, 1.0],
    [0.0, 0.0],
    [1.0, 1.0],
    [1.0, 1.0],
    [0.0, 0.0],
    [1.0, 0.0],
];

// TODO: Most of the information in the Gltf is not necessary for things to function. Spawning one
// just transplants the entities from the Scene. Only the information I've needed has been added, maybe
// do the rest so it's proper?
//
// A substitute for creating proper models for items that just need simple models, e.g. blocks.
// Let's users specify only the necessary information needed to cobble together a gltf at runtime.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum JsonModel {
    Block {
        // image file names
        top: String,
        bottom: String,
        left: String,
        right: String,
        front: String,
        back: String,
    },
}

impl JsonModel {
    fn build_gltf(&self, asset_server: &AssetServer) -> Gltf {
        match self {
            Self::Block { .. } => self.build_block_gltf(asset_server),
        }
    }

    fn build_block_gltf(&self, asset_server: &AssetServer) -> Gltf {
        let Self::Block {
            top,
            bottom,
            left,
            right,
            front,
            back,
        } = self;
        let ordered_names = [top, bottom, left, right, front, back];

        let mut gltf_meshes = Vec::new();

        let mut world = World::new();
        world
            .spawn(SpatialBundle::INHERITED_IDENTITY)
            .with_children(|parent| {
                for i in 0..6 {
                    let mut mesh = Mesh::new(
                        PrimitiveTopology::TriangleList,
                        RenderAssetUsages::default(),
                    );
                    mesh.insert_attribute(
                        Mesh::ATTRIBUTE_POSITION,
                        BLOCK_MODEL_VERTICES[i].to_vec(),
                    );
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, BLOCK_MODEL_UVS.to_vec());
                    mesh.compute_flat_normals();
                    let mesh_handle = asset_server.add(mesh);
                    let material_handle = asset_server.add(StandardMaterial {
                        base_color_texture: Some(
                            asset_server.load(BLOCK_TEXTURE_PATH.to_owned() + ordered_names[i]),
                        ),
                        ..default()
                    });
                    gltf_meshes.push(asset_server.add(GltfMesh {
                        primitives: vec![GltfPrimitive {
                            mesh: mesh_handle.clone(),
                            material: None,
                            extras: None,
                            material_extras: None,
                        }],
                        extras: None,
                    }));
                    parent.spawn((
                        SpatialBundle::INHERITED_IDENTITY,
                        mesh_handle,
                        material_handle,
                    ));
                }
            });

        let scene_handle = asset_server.add(Scene { world });

        Gltf {
            scenes: vec![scene_handle.clone()],
            named_scenes: HashMap::new(),
            meshes: gltf_meshes,
            named_meshes: HashMap::new(),
            materials: Vec::new(),
            named_materials: HashMap::new(),
            nodes: Vec::new(),
            named_nodes: HashMap::new(),
            default_scene: Some(scene_handle),
            animations: Vec::new(),
            named_animations: HashMap::new(),
            source: None,
        }
    }

    // TODO: This animation is bad, blender can be used to make a good one. The camera of blender
    // is hard to make match up with the one for bevy, idk why. A good approximation can be found
    // in one of the blender files iirc
    fn click_animation() -> AnimationClip {
        let mut animation = AnimationClip::default();
        let name = Name::new("model");
        animation.add_curve_to_path(
            EntityPath {
                parts: vec![name.clone()],
            },
            VariableCurve {
                keyframe_timestamps: vec![0.0, 0.083333336, 0.125, 0.16666667, 0.20833333],
                keyframes: Keyframes::Translation(vec![
                    Vec3::new(0.1020781, -0.13220775, -0.10700002),
                    Vec3::new(0.076120734, -0.114703014, -0.10700002),
                    Vec3::new(0.050163373, -0.09719828, -0.10700002),
                    Vec3::new(0.07612074, -0.11470301, -0.10700002),
                    Vec3::new(0.1020781, -0.13220775, -0.10700002),
                ]),
                interpolation: Interpolation::Linear,
            },
        );
        animation.add_curve_to_path(
            EntityPath {
                parts: vec![name.clone()],
            },
            VariableCurve {
                keyframe_timestamps: vec![0.0, 0.083333336, 0.125, 0.16666667, 0.20833333],
                keyframes: Keyframes::Rotation(vec![
                    Quat::from_xyzw(-0.60538566, 0.31365922, 0.32767585, 0.65402955),
                    Quat::from_xyzw(-0.7988094, 0.3206745, 0.33568513, 0.3826055),
                    Quat::from_xyzw(-0.86302054, 0.33542478, 0.37329403, 0.05776981),
                    Quat::from_xyzw(-0.7988095, 0.32067442, 0.33568496, 0.38260534),
                    Quat::from_xyzw(-0.60538566, 0.31365922, 0.32767585, 0.65402955),
                ]),
                interpolation: Interpolation::Linear,
            },
        );
        animation.add_curve_to_path(
            EntityPath {
                parts: vec![name.clone()],
            },
            VariableCurve {
                keyframe_timestamps: vec![0.0, 0.083333336, 0.125, 0.16666667, 0.20833333],
                keyframes: Keyframes::Scale(vec![
                    Vec3::new(0.07331951, 0.07331951, 0.07331952),
                    Vec3::new(0.07331951, 0.07331951, 0.07331952),
                    Vec3::new(0.07331951, 0.07331951, 0.07331951),
                    Vec3::new(0.07331951, 0.07331951, 0.07331952),
                    Vec3::new(0.07331951, 0.07331951, 0.07331952),
                ]),
                interpolation: Interpolation::Linear,
            },
        );
        animation
    }
}

// TODO: Not a good name
#[derive(Component, Default)]
pub struct GltfAnimationPlayers {
    pub main: Option<Entity>,
    _bones: HashMap<usize, Entity>,
}

// TODO: I think 'main' should be the first animation player found, or match the name against the
// filename or something. 'main' should also be added to bones?
// TODO: bones
fn register_gltf_animation_players(
    mut commands: Commands,
    added_scenes: Query<(Entity, &Children), (With<Handle<Scene>>, Added<Children>, With<Model>)>,
    children: Query<&Children>,
    animation_players: Query<(Entity, &Name), With<AnimationPlayer>>,
) {
    for (entity, root_children) in added_scenes.iter() {
        let mut gltf_animation_players = GltfAnimationPlayers::default();

        for root_child in root_children {
            let node_children = children.get(*root_child).unwrap();
            for node_child in node_children {
                if let Ok((animation_player_entity, _name)) = animation_players.get(*node_child) {
                    gltf_animation_players.main = Some(animation_player_entity);
                }
            }
        }

        commands.entity(entity).try_insert(gltf_animation_players);
    }
}
