use bevy::math::U8Vec3;

/// Represents a local block position within a chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalBlockPos(U8Vec3);

impl LocalBlockPos {
    #[must_use]
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        Self(U8Vec3::new(x, y, z))
    }

    #[must_use]
    pub const fn from_u8vec3(v: U8Vec3) -> Self {
        Self(v)
    }

    #[must_use]
    pub const fn x(self) -> u8 {
        self.0.x
    }

    #[must_use]
    pub const fn y(self) -> u8 {
        self.0.y
    }

    #[must_use]
    pub const fn z(self) -> u8 {
        self.0.z
    }

    #[must_use]
    pub fn index(self) -> usize {
        let masked = (self.0 & U8Vec3::splat(15)).as_usizevec3();
        masked.x << 8 | masked.z << 4 | masked.y
    }
}
