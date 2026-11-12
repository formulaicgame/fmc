use std::ops::{Add, AddAssign, Sub, SubAssign};

use bevy::{math::DVec3, prelude::*};

use crate::{game_state::GameState, player::Head};

mod chunk;
mod chunk_manager;
mod world_map;

pub use chunk::{Chunk, ChunkFace};
pub use chunk_manager::NewChunkEvent;
pub use world_map::WorldMap;

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(world_map::WorldMapPlugin)
            .add_plugins(chunk_manager::ChunkManagerPlugin)
            .insert_resource(Origin(ChunkPosition::default()))
            .add_systems(
                PostUpdate,
                update_origin.run_if(in_state(GameState::Playing)),
            );
    }
}

// TODO: Transforms could have been made to be f64 as with the server, but I don't know
// enough about the rendering stuff to replace Transform. Instead this litters conversions all over
// the place.
//
// For entities that use a Transform, an offset is needed to preserve the precision of f32s. This
// is updated to be the chunk position of the player every time the player moves between chunk
// borders.
#[derive(Resource, Deref, DerefMut, Clone, Copy)]
pub struct Origin(pub ChunkPosition);

impl Origin {
    pub fn as_ivec3(&self) -> IVec3 {
        self.0.0
    }

    pub fn to_local(&self, position: DVec3) -> Vec3 {
        (position - self.as_dvec3()).as_vec3()
    }

    pub fn to_global(&self, position: Vec3) -> DVec3 {
        self.as_dvec3() + position.as_dvec3()
    }
}

#[derive(Component)]
pub struct MovesWithOrigin;

#[derive(Component, Deref, DerefMut, Hash, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ChunkPosition(pub IVec3);

impl ChunkPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self::from(IVec3::new(x, y, z))
    }

    pub fn as_ivec3(&self) -> IVec3 {
        self.0
    }

    pub fn offset(&self, face: ChunkFace) -> Self {
        let size = Chunk::SIZE as i32;
        Self(
            self.0
                + match face {
                    ChunkFace::Front => IVec3::new(0, 0, size),
                    ChunkFace::Back => IVec3::new(0, 0, -size),
                    ChunkFace::Right => IVec3::new(size, 0, 0),
                    ChunkFace::Left => IVec3::new(-size, 0, 0),
                    ChunkFace::Top => IVec3::new(0, size, 0),
                    ChunkFace::Bottom => IVec3::new(0, -size, 0),
                    ChunkFace::None => IVec3::ZERO,
                },
        )
    }
}

impl From<BlockPosition> for ChunkPosition {
    fn from(value: BlockPosition) -> Self {
        Self::from(value.0)
    }
}

impl From<DVec3> for ChunkPosition {
    fn from(value: DVec3) -> Self {
        Self::from(BlockPosition::from(value))
    }
}

impl From<IVec3> for ChunkPosition {
    fn from(value: IVec3) -> Self {
        Self(value & !(Chunk::SIZE as i32 - 1))
    }
}

impl Add<ChunkPosition> for ChunkPosition {
    type Output = ChunkPosition;

    fn add(self, rhs: ChunkPosition) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<ChunkPosition> for ChunkPosition {
    type Output = ChunkPosition;

    fn sub(self, rhs: ChunkPosition) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Component, Deref, DerefMut, Hash, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BlockPosition(pub IVec3);

impl BlockPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub fn as_chunk_index(&self) -> usize {
        let position = self.0 & (Chunk::SIZE as i32 - 1);
        (position.x << 8 | position.z << 4 | position.y) as usize
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
        Self(IVec3 {
            x: (index >> 8) as i32,
            z: (index >> 4 & MASK) as i32,
            y: (index & MASK) as i32,
        })
    }
}

impl From<ChunkPosition> for BlockPosition {
    fn from(value: ChunkPosition) -> Self {
        Self(value.0)
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
    fn add_assign(&mut self, rhs: BlockPosition) {
        self.0 += rhs.0;
    }
}

impl AddAssign<IVec3> for BlockPosition {
    fn add_assign(&mut self, rhs: IVec3) {
        self.0 += rhs;
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
    fn sub_assign(&mut self, rhs: BlockPosition) {
        self.0 -= rhs.0;
    }
}

impl SubAssign<IVec3> for BlockPosition {
    fn sub_assign(&mut self, rhs: IVec3) {
        self.0 -= rhs;
    }
}

pub fn update_origin(
    mut origin: ResMut<Origin>,
    mut positions: ParamSet<(
        Query<&GlobalTransform, (Changed<GlobalTransform>, With<Head>)>,
        Query<&mut Transform, With<MovesWithOrigin>>,
    )>,
) {
    let for_lifetime = positions.p0();
    let player_transform = if let Ok(t) = for_lifetime.single() {
        t
    } else {
        return;
    };

    let true_translation = origin.to_global(player_transform.translation());
    let new_origin = ChunkPosition::from(true_translation);

    if new_origin == origin.0 {
        return;
    };

    let change = (new_origin - origin.0).as_vec3();
    for mut transform in positions.p1().iter_mut() {
        transform.translation -= change;
    }

    origin.0 = new_origin;
}
