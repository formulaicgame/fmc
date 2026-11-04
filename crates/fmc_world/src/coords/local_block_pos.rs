use crate::coords::constants::{LOCAL_MASK, LOCAL_SHIFT, PROTO_CHUNK_SIZE};

pub struct LocalBlockPos(u16);

impl LocalBlockPos {
    #[must_use]
    pub fn new(x: u8, y: u8, z: u8) -> Self {
        debug_assert!(
            (0..PROTO_CHUNK_SIZE).contains(&x),
            "x-side length too long: {x}"
        );
        debug_assert!(
            (0..PROTO_CHUNK_SIZE).contains(&y),
            "y-side length too long: {y}"
        );
        debug_assert!(
            (0..PROTO_CHUNK_SIZE).contains(&z),
            "z-side length too long: {z}"
        );

        let [x, y, z] = [x, y, z].map(Self::mask);
        Self(Self::pack(x, y, z))
    }

    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn x(self) -> u8 {
        (self.0 & u16::from(LOCAL_MASK)) as u8
    }

    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn y(self) -> u8 {
        ((self.0 >> LOCAL_SHIFT) & u16::from(LOCAL_MASK)) as u8
    }

    #[must_use]
    pub fn z(self) -> u8 {
        ((self.0 >> (2 * LOCAL_SHIFT)) & u16::from(LOCAL_MASK)) as u8
    }

    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    const fn pack(x: u16, y: u16, z: u16) -> u16 {
        z << (2 * LOCAL_SHIFT) | y << LOCAL_SHIFT | x
    }

    fn mask(v: u8) -> u16 {
        u16::from(v & LOCAL_MASK)
    }
}
