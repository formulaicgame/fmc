use bevy::{math::DVec3, prelude::*};

use fmc_networking_derive::{ClientBound, NetworkMessage};
use serde::{Deserialize, Serialize};

/// Spawn a new model.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct NewModel {
    /// Id used to reference it when updating. If the same id is sent twice, the old model will be
    /// replaced.
    pub id: u32,
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

/// Delete an existing model.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct DeleteModel {
    /// Id of the model
    pub id: u32,
}

/// Update the asset used by a model.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdateAsset {
    /// Id of the model
    pub id: u32,
    /// Asset id
    pub asset: u32,
}

/// Update the transform of a model.
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct ModelUpdateTransform {
    /// Id of the model
    pub id: u32,
    /// Position update
    pub position: DVec3,
    /// Rotation update
    pub rotation: Quat,
    /// Scale update
    pub scale: Vec3,
}

/// Play an animation of a model
#[derive(NetworkMessage, ClientBound, Serialize, Deserialize, Debug, Clone)]
pub struct ModelPlayAnimation {
    /// Id of the model
    pub model_id: u32,
    /// Index of the animation
    pub animation_index: u32,
    /// Makes the animation loop
    pub repeat: bool,
}
