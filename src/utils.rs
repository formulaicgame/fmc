#[derive(Default, Debug, Clone)]
pub struct Rng {
    seed: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        let seed = self.seed.wrapping_add(0x2d35_8dcc_aa6c_78a5);
        self.seed = seed;
        let t = u128::from(seed) * u128::from(seed ^ 0x8bb8_4b93_962e_acc9);
        return ((t as u64) ^ (t >> 64) as u64) as u32;
    }

    pub fn next_f32(&mut self) -> f32 {
        let seed = self.seed.wrapping_add(0x2d35_8dcc_aa6c_78a5);
        self.seed = seed;
        let t = u128::from(seed) * u128::from(seed ^ 0x8bb8_4b93_962e_acc9);
        let result = ((t as u64) ^ (t >> 64) as u64) as u32;
        // Only want 23 bits of the result for the mantissa, rest is discarded and replaced
        // with exponent of 127 so the result is in range 1..2 then -1 to move the range down
        // to 0..1
        f32::from_bits((result >> 9) | (127 << 23)) - 1.0
    }
}
