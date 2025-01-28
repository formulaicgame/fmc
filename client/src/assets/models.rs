use bevy::{
    animation::{animated_field, AnimationTarget, AnimationTargetId},
    gltf::{Gltf, GltfMesh, GltfPrimitive},
    prelude::*,
    render::{mesh::MeshAabb, render_asset::RenderAssetUsages, render_resource::PrimitiveTopology},
    utils::HashMap,
};

use fmc_protocol::messages;
use serde::Deserialize;

use crate::{game_state::GameState, networking::NetworkClient};

const MODEL_PATH: &str = "server_assets/active/textures/models/";
const BLOCK_TEXTURE_PATH: &str = "server_assets/active/textures/blocks/";

// Model asset ids are provided by the server on connection
pub type ModelAssetId = u32;

pub(super) struct ModelPlugin;
impl Plugin for ModelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                construct_animations.run_if(resource_exists::<Models>),
                transfer_animation_targets.run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

// TODO: Keeping all the models loaded will probably be too expensive. Need to be able to
// load them when needed, but still load them all fully once and validate so it can disconnect early.
//
/// A map from server's model ids to their configs
#[derive(Resource)]
pub struct Models {
    pub id2config: std::collections::HashMap<u32, ModelConfig>,
    filename2id: std::collections::HashMap<String, u32>,
}

impl Models {
    pub fn get_config(&self, id: &ModelAssetId) -> Option<&ModelConfig> {
        return self.id2config.get(&id);
    }

    pub fn get_id_by_filename(&self, filename: &str) -> Option<ModelAssetId> {
        return self.filename2id.get(filename).cloned();
    }

    pub fn iter(&self) -> std::collections::hash_map::Values<ModelAssetId, ModelConfig> {
        return self.id2config.values();
    }
}

pub struct ModelConfig {
    pub gltf_handle: Handle<Gltf>,
    pub animation_graph: Option<Handle<AnimationGraph>>,
    pub animations: Vec<AnimationNodeIndex>,
    pub named_animations: HashMap<String, AnimationNodeIndex>,
}

#[derive(Component)]
pub enum Model {
    Asset(ModelAssetId),
    Custom,
}

#[derive(Resource)]
struct LoadingModels {
    models: HashMap<AssetId<Gltf>, ModelAssetId>,
}

pub(super) fn load_models(
    mut commands: Commands,
    net: Res<NetworkClient>,
    server_config: Res<messages::ServerConfig>,
    asset_server: Res<AssetServer>,
) {
    let directory = match std::fs::read_dir(MODEL_PATH) {
        Ok(dir) => dir,
        Err(e) => {
            net.disconnect(&format!(
                "Misconfigured assets: Failed to read model directory at '{}'\n Error: {}",
                MODEL_PATH, e
            ));
            return;
        }
    };

    // This is the genertic animation applied to runtime generated models. Models loaded from gltf
    // files should supply their own animation.
    let click_animation = asset_server.add(JsonModel::click_animation());
    let equip_animation = asset_server.add(JsonModel::equip_animation());
    let (block_animation_graph, block_animation_indices) =
        AnimationGraph::from_clips([click_animation.clone(), equip_animation.clone()]);
    let block_animation_graph = asset_server.add(block_animation_graph);

    let mut model_configs = Models {
        id2config: std::collections::HashMap::new(),
        filename2id: std::collections::HashMap::new(),
    };
    let mut loading_models = LoadingModels {
        models: HashMap::new(),
    };

    for dir_entry in directory {
        let path = match dir_entry {
            Ok(d) => d.path(),
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: Failed to read the file path of a model\n\
                    Error: {}",
                    e
                ));
                return;
            }
        };

        let model_name = path.file_stem().unwrap().to_string_lossy().into_owned();

        let Some(model_id) = server_config.model_ids.get(&model_name) else {
            net.disconnect("Misconfigured assets: There's a model named '{}' in the assets: but the server didn't send an id for it.");
            return;
        };

        let Some(extension) = path.extension() else {
            net.disconnect(&format!(
                "Invalid model file at '{}', the file is missing its extension. \
                    Should be one of 'json', 'gltf' or 'glb'.",
                path.display()
            ));
            return;
        };

        let model_config = if extension == "json" {
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
                        "Misconfigured assets: Could not parse model at '{}'\nError: {e}",
                        path.display()
                    ));
                    return;
                }
            };
            let mut gltf = json_model.build_gltf(asset_server.as_ref());
            gltf.animations.push(click_animation.clone());
            gltf.named_animations
                .insert("left_click".into(), click_animation.clone());
            gltf.animations.push(equip_animation.clone());
            gltf.named_animations
                .insert("equip".into(), equip_animation.clone());
            let gltf_handle = asset_server.add(gltf);

            loading_models.models.insert(gltf_handle.id(), *model_id);

            ModelConfig {
                gltf_handle,
                animation_graph: Some(block_animation_graph.clone()),
                animations: block_animation_indices.clone(),
                named_animations: HashMap::from([
                    ("left_click".to_owned(), block_animation_indices[0]),
                    ("equip".to_owned(), block_animation_indices[1]),
                ]),
            }
        } else if extension == "glb" || extension == "gltf" {
            let gltf_handle = asset_server.load(path);

            loading_models.models.insert(gltf_handle.id(), *model_id);

            ModelConfig {
                gltf_handle,
                animation_graph: None,
                animations: Vec::new(),
                named_animations: HashMap::new(),
            }
        } else {
            //net.disconnect(message);
            panic!();
            return;
        };

        model_configs.filename2id.insert(model_name, *model_id);
        model_configs.id2config.insert(*model_id, model_config);
    }

    for (name, id) in server_config.model_ids.iter() {
        if !model_configs.id2config.contains_key(id) {
            net.disconnect(&format!(
                "Misconfigured assets: Missing model, no model with the name '{}', make sure it is part of the assets",
                name
            ));
        }
    }

    commands.insert_resource(loading_models);
    commands.insert_resource(model_configs);
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
        let mut entity_commands = world.spawn_empty();
        let entity = entity_commands.id();
        entity_commands
            .insert((
                Transform::default(),
                Visibility::default(),
                AnimationPlayer::default(),
                AnimationTarget {
                    id: AnimationTargetId::from_name(&Name::new("block_model")),
                    player: entity,
                },
            ))
            .with_children(|parent| {
                let mut gltf_mesh = GltfMesh {
                    index: 0,
                    name: String::from("block"),
                    primitives: Vec::new(),
                    extras: None,
                };

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
                    gltf_mesh.primitives.push(GltfPrimitive {
                        index: i,
                        name: i.to_string(),
                        parent_mesh_index: i,
                        mesh: mesh_handle.clone(),
                        material: Some(material_handle.clone()),
                        extras: None,
                        material_extras: None,
                    });
                    parent.spawn((
                        Transform::default(),
                        Visibility::default(),
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(material_handle),
                    ));
                }

                gltf_meshes.push(asset_server.add(gltf_mesh));
            });

        let scene_handle = asset_server.add(Scene { world });

        // TODO: Fill out the gltf properly. I've just included the values I need since the gltf is
        // only used for reference, not spawning.
        Gltf {
            scenes: vec![scene_handle.clone()],
            named_scenes: HashMap::new(),
            meshes: gltf_meshes,
            named_meshes: HashMap::new(),
            materials: Vec::new(),
            named_materials: HashMap::new(),
            nodes: Vec::new(),
            named_nodes: HashMap::new(),
            skins: Vec::new(),
            named_skins: HashMap::new(),
            default_scene: Some(scene_handle),
            animations: Vec::new(),
            named_animations: HashMap::new(),
            source: None,
        }
    }

    // NOTE: If you want to make this better there's a blender file called "block_template.blend"
    // in the *server* implementation
    fn click_animation() -> AnimationClip {
        let mut animation = AnimationClip::default();
        let name = Name::new("block_model");
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                UnevenSampleAutoCurve::new(
                    [
                        0.0,
                        0.016666668,
                        0.033333335,
                        0.05,
                        0.06666667,
                        0.083333336,
                        0.1,
                        0.11666667,
                        0.13333334,
                        0.15,
                        0.16666667,
                        0.18333334,
                        0.2,
                        0.21666667,
                    ]
                    .into_iter()
                    .zip([
                        Vec3::new(0.045858342, -0.13410479, -0.14700001),
                        Vec3::new(0.038743485, -0.12983587, -0.14700001),
                        Vec3::new(0.031628624, -0.12556696, -0.14700001),
                        Vec3::new(0.024513766, -0.12129804, -0.14700001),
                        Vec3::new(0.016800273, -0.13960254, -0.14700001),
                        Vec3::new(0.015463241, -0.1475193, -0.14700001),
                        Vec3::new(0.016409533, -0.15253721, -0.14700001),
                        Vec3::new(0.018662963, -0.15581624, -0.14700001),
                        Vec3::new(0.021805331, -0.1577653, -0.14700001),
                        Vec3::new(0.025615111, -0.15851778, -0.14700001),
                        Vec3::new(0.029963113, -0.15802298, -0.14700001),
                        Vec3::new(0.034774512, -0.15598658, -0.14700001),
                        Vec3::new(0.04002135, -0.1514692, -0.14700001),
                        Vec3::new(0.045858342, -0.13410479, -0.14700001),
                    ]),
                )
                .unwrap(),
            ),
        );
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::rotation),
                UnevenSampleAutoCurve::new(
                    [
                        0.0,
                        0.016666668,
                        0.033333335,
                        0.05,
                        0.06666667,
                        0.083333336,
                        0.1,
                        0.11666667,
                        0.13333334,
                        0.15,
                        0.16666667,
                        0.18333334,
                        0.2,
                        0.21666667,
                    ]
                    .into_iter()
                    .zip([
                        Quat::from_xyzw(0.013405943, 0.453133, 0.020592665, 0.8911042),
                        Quat::from_xyzw(-0.11353387, 0.4639351, 0.036938243, 0.8777869),
                        Quat::from_xyzw(-0.24070297, 0.4669857, 0.05293155, 0.84922594),
                        Quat::from_xyzw(-0.3616851, 0.4621311, 0.06776566, 0.8068622),
                        Quat::from_xyzw(-0.44845086, 0.45262963, 0.07820215, 0.76674813),
                        Quat::from_xyzw(-0.5310297, 0.43890235, 0.087908566, 0.71947503),
                        Quat::from_xyzw(-0.5313901, 0.4392143, 0.079550505, 0.7199911),
                        Quat::from_xyzw(-0.53129196, 0.439573, 0.07118596, 0.72071975),
                        Quat::from_xyzw(-0.52861947, 0.44036335, 0.06285859, 0.7229733),
                        Quat::from_xyzw(-0.5177648, 0.44255105, 0.05464625, 0.73012465),
                        Quat::from_xyzw(-0.48723018, 0.44772276, 0.046613112, 0.748317),
                        Quat::from_xyzw(-0.41525686, 0.4570547, 0.038671818, 0.78559995),
                        Quat::from_xyzw(-0.263162, 0.4663653, 0.030317979, 0.84399647),
                        Quat::from_xyzw(0.013405943, 0.453133, 0.020592665, 0.8911042),
                    ]),
                )
                .unwrap(),
            ),
        );
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::scale),
                UnevenSampleAutoCurve::new([
                    (0.0, Vec3::new(0.07331951, 0.07331951, 0.073319525)),
                    (0.1, Vec3::new(0.07331951, 0.07331951, 0.073319525)),
                ])
                .unwrap(),
            ),
        );

        return animation;
    }

    fn equip_animation() -> AnimationClip {
        let mut mesh = Mesh::from(Cuboid {
            half_size: Vec3::splat(0.5),
        });
        mesh.scale_by(Vec3::new(0.07331951, 0.07331951, 0.073319525));
        mesh.rotate_by(Quat::from_xyzw(
            0.013405943,
            0.453133,
            0.020592665,
            0.8911042,
        ));
        let aabb = mesh.compute_aabb().unwrap();
        let height = aabb.half_extents.y * 2.0;

        let mut animation = AnimationClip::default();
        let name = Name::new("block_model");
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                UnevenSampleAutoCurve::new([0.0, 0.15].into_iter().zip([
                    Vec3::new(0.045858342, -0.13410479, -0.14700001) - Vec3::new(0.0, height, 0.0),
                    Vec3::new(0.045858342, -0.13410479, -0.14700001),
                ]))
                .unwrap(),
            ),
        );
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::rotation),
                UnevenSampleAutoCurve::new([0.0, 0.15].into_iter().zip([
                    Quat::from_xyzw(0.013405943, 0.453133, 0.020592665, 0.8911042),
                    Quat::from_xyzw(0.013405943, 0.453133, 0.020592665, 0.8911042),
                ]))
                .unwrap(),
            ),
        );
        animation.add_curve_to_target(
            AnimationTargetId::from_name(&name),
            AnimatableCurve::new(
                animated_field!(Transform::scale),
                UnevenSampleAutoCurve::new([
                    (0.0, Vec3::new(0.07331951, 0.07331951, 0.073319525)),
                    (0.1, Vec3::new(0.07331951, 0.07331951, 0.073319525)),
                ])
                .unwrap(),
            ),
        );

        return animation;
    }
}

// Points all animation targets to one central AnimationPlayer at the root entity.
fn transfer_animation_targets(
    children: Query<&Children>,
    mut animation_targets: Query<&mut AnimationTarget>,
    mut added_scenes: Query<Entity, (With<AnimationPlayer>, With<SceneRoot>, Added<Children>)>,
) {
    fn change_animation_target(
        root: Entity,
        child: Entity,
        children: &Query<&Children>,
        animation_targets: &mut Query<&mut AnimationTarget>,
    ) {
        if let Ok(mut animation_target) = animation_targets.get_mut(child) {
            animation_target.player = root;
        }

        if let Ok(node_children) = children.get(child) {
            for node_child in node_children {
                change_animation_target(root, *node_child, children, animation_targets)
            }
        }
    }

    for root_entity in added_scenes.iter_mut() {
        change_animation_target(root_entity, root_entity, &children, &mut animation_targets);
    }
}

// TODO: Because loading gltf assets is async this can get delayed until the game has started which would cause
// a panic. The models should preferably be loaded fully while loading the assets.
//
// Models that are loaded through the asset server need to have their animation graphs constructed
// after the gltf has been loaded, as well as to add any animations that should be generated.
fn construct_animations(
    mut models: ResMut<Models>,
    mut loading_models: ResMut<LoadingModels>,
    mut gltfs: ResMut<Assets<Gltf>>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut asset_events: EventReader<AssetEvent<Gltf>>,
) {
    for event in asset_events.read() {
        let AssetEvent::Added { id } = event else {
            continue;
        };

        let Some(model_id) = loading_models.models.remove(id) else {
            continue;
        };

        let gltf = gltfs.get_mut(*id).unwrap();
        let model = models.id2config.get_mut(&model_id).unwrap();
        // We have to pre-allocate because the order in named_animations does not correspond to the
        // one in 'animations'
        model.animations = vec![AnimationNodeIndex::default(); gltf.animations.len()];

        let mut animation_graph = AnimationGraph::new();
        for (name, animation_clip) in gltf.named_animations.iter() {
            let index = gltf
                .animations
                .iter()
                .position(|a| a == animation_clip)
                .unwrap();
            let node_index =
                animation_graph.add_clip(animation_clip.clone(), 1.0, animation_graph.root);

            model.animations[index] = node_index;
            model.named_animations.insert(name.to_string(), node_index);
        }

        if !model.named_animations.contains_key("equip") {
            // This will build an equip animation if there is a "left_click" animation available
            // build_equip_animation(
            //     model,
            //     &mut gltfs,
            //     &gltf_meshes,
            //     &meshes,
            //     &mut animation_clips,
            //     &mut animation_graph,
            // )
        }

        model.animation_graph = Some(asset_server.add(animation_graph));
    }
}

// TODO: Bobbing animation.
// #[inline]
// fn build_equip_animation(
//     model: &mut ModelConfig,
//     gltfs: &mut Assets<Gltf>,
//     gltf_meshes: &Assets<GltfMesh>,
//     meshes: &Assets<Mesh>,
//     animation_clips: &mut Assets<AnimationClip>,
//     animation_graph: &mut AnimationGraph,
// ) {
//     // The GltfMesh making up the model may consist of many different meshes. We need to know the
//     // combined size of them to decide the height of the model when equipped. The height decides how far
//     // we need to lower the model to make it go out of frame.
//     let mut combined_mesh = Mesh::new(
//         PrimitiveTopology::TriangleList,
//         RenderAssetUsages::default(),
//     );
//     combined_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]]);
//
//     let gltf = gltfs.get_mut(&model.gltf_handle).unwrap();
//     let gltf_mesh = gltf_meshes.get(&gltf.meshes[0]).unwrap();
//     for primitive in gltf_mesh.primitives.iter() {
//         let mesh = meshes.get(&primitive.mesh).unwrap();
//         // TODO: This can panic if the attributes of the meshes aren't compatible. I don't know if
//         // the gltf loader does any checks. We can do this manually, we only care about the default
//         // f32x3
//         combined_mesh.merge(mesh);
//     }
//
//     // The left click animation's first frame decides the default transform of the model when
//     // equipped, so this should be the end point of the equip animation.
//     let Some(left_click_animation) = gltf
//         .named_animations
//         .get("left_click")
//         .and_then(|handle| animation_clips.get(handle))
//     else {
//         return;
//     };
//
//     if left_click_animation.curves().is_empty() {
//         return;
//     }
//
//     let (target_id, curves) = left_click_animation.curves().iter().next().unwrap();
//
//     for curve in curves.iter() {
//         match &curve.keyframes {
//             Keyframes::Rotation(rotations) => {
//                 combined_mesh.rotate_by(rotations[0]);
//             }
//             Keyframes::Scale(scales) => {
//                 combined_mesh.scale_by(scales[0]);
//             }
//             _ => (),
//         }
//     }
//
//     let aabb = combined_mesh.compute_aabb().unwrap();
//     let height = aabb.half_extents.y * 2.0;
//
//     let mut equip_animation = AnimationClip::default();
//     curves.iter().for_each(|curve| {
//         equip_animation.add_curve_to_target(
//             *target_id,
//             VariableCurve {
//                 keyframe_timestamps: vec![0.0, 0.15],
//                 keyframes: match &curve.keyframes {
//                     Keyframes::Translation(translations) => Keyframes::Translation(vec![
//                         translations[0] - Vec3::new(0.0, height, 0.0),
//                         translations[0],
//                     ]),
//                     Keyframes::Rotation(rotations) => Keyframes::Rotation(vec![rotations[0]; 2]),
//                     Keyframes::Scale(scales) => Keyframes::Scale(vec![scales[0]; 2]),
//                     Keyframes::Weights(weights) => Keyframes::Weights(vec![weights[0]; 2]),
//                 },
//                 interpolation: curve.interpolation.clone(),
//             },
//         )
//     });
//
//     let equip_handle = animation_clips.add(equip_animation);
//     gltf.animations.push(equip_handle.clone());
//     gltf.named_animations
//         .insert("equip".into(), equip_handle.clone());
//
//     let node_index = animation_graph.add_clip(equip_handle, 1.0, animation_graph.root);
//     model.animations.push(node_index);
//     model
//         .named_animations
//         .insert("equip".to_owned(), node_index);
// }
