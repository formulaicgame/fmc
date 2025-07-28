use bevy::{math::DVec3, prelude::*};

use fmc_protocol_derive::ClientBound;
use serde::{Deserialize, Serialize};

/// Spawn a new model.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct NewModel {
    /// Id used to reference it when updating. If the same id is sent twice, the old model will be
    /// replaced.
    pub model_id: u32,
    /// Inherit position/rotation from another model. If the parent transform changes, this model
    /// will change in the same way.
    pub parent_id: Option<u32>,
    /// Position of the model
    pub position: DVec3,
    /// Rotation of the model
    pub rotation: Quat,
    /// Scale of the model
    pub scale: Vec3,
    /// Id of asset that should be used to render the model
    pub asset: u32,
}

/// Spawn a custom model that is not part of the assets
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct SpawnCustomModel {
    /// Id used to reference it when updating. If the same id is sent twice, the old model will be
    /// replaced.
    pub model_id: u32,
    /// Inherit position/rotation from another model. If the parent transform changes, this model
    /// will change in the same way.
    pub parent_id: Option<u32>,
    /// Position of the model
    pub position: DVec3,
    /// Rotation of the model
    pub rotation: Quat,
    /// Scale of the model
    pub scale: Vec3,
    /// Mesh Indices
    pub mesh_indices: Vec<u32>,
    /// Mesh vertices
    pub mesh_vertices: Vec<[f32; 3]>,
    /// Mesh normals
    pub mesh_normals: Vec<[f32; 3]>,
    /// Texture uvs
    pub mesh_uvs: Option<Vec<[f32; 2]>>,
    /// Color texture of the mesh
    pub material_color_texture: Option<String>,
    /// Texture used for parallax mapping
    pub material_parallax_texture: Option<String>,
    /// Alpha blend mode, 0 = Opaque, 1 = mask, 2 = blend
    pub material_alpha_mode: u8,
    /// Alpha channel cutoff if the blend mode is Mask
    pub material_alpha_cutoff: f32,
    /// Render mesh from both sides
    pub material_double_sided: bool,
}

impl Default for SpawnCustomModel {
    fn default() -> Self {
        Self {
            model_id: 0,
            parent_id: None,
            position: DVec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh_indices: Vec::new(),
            mesh_vertices: Vec::new(),
            mesh_normals: Vec::new(),
            mesh_uvs: None,
            material_color_texture: None,
            material_parallax_texture: None,
            material_alpha_mode: 0,
            material_alpha_cutoff: 0.0,
            material_double_sided: false,
        }
    }
}

/// Delete an existing model.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct DeleteModel {
    /// Id of the model
    pub model_id: u32,
}

/// Update the asset used by a model.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdateAsset {
    /// Id of the model
    pub model_id: u32,
    /// Asset id
    pub asset: u32,
}

/// Update the transform of a model.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdateTransform {
    /// Id of the model
    pub model_id: u32,
    /// Position update
    pub position: DVec3,
    /// Rotation update
    pub rotation: Quat,
    /// Scale update
    pub scale: Vec3,
}

/// Play an animation of a model
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct ModelPlayAnimation {
    /// Id of the model
    pub model_id: u32,
    /// Index of the animation
    pub animation_index: u32,
    /// Restart the animation if it's already playing
    pub restart: bool,
    /// Makes the animation loop
    pub repeat: bool,
    /// Replace an old animation with this one,
    /// (animation_index, transition_time)
    pub transition: Option<(u32, f32)>,
}

/// Change the base color of a model's material
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct ModelColor {
    /// Id of the model
    pub model_id: u32,
    /// Color formated as hex, "RRGGBBAA"
    pub color: String,
}
