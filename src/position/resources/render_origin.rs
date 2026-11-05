use bevy::{math::DVec3, prelude::*};
use fmc_world::{coords::ChunkPos, transform::VoxelTransform};

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug, Deref, DerefMut)]
pub struct RenderOrigin(pub ChunkPos);

impl RenderOrigin {
    #[deprecated]
    pub fn to_local(self, pos: DVec3) -> Vec3 {
        (pos - self.corner()).as_vec3()
    }

    #[deprecated]
    pub fn to_global(self, render: Vec3) -> VoxelTransform {
        VoxelTransform::from_chunk_local(self.0, render)
    }
}
