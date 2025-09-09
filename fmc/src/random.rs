use std::ops::{Range, RangeInclusive};

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

    pub fn range_u32(&mut self, range: impl RngRange<u32>) -> u32 {
        range.start() + self.next_u32() % (range.end() - range.start() + 1)
    }

    pub fn range_f32(&mut self, range: impl RngRange<f32>) -> f32 {
        range.start() + self.next_f32() * (range.end() - range.start())
    }
}

pub trait RngRange<T> {
    fn start(&self) -> T;
    fn end(&self) -> T;
}

impl RngRange<f32> for Range<f32> {
    fn start(&self) -> f32 {
        self.start
    }

    fn end(&self) -> f32 {
        self.end
    }
}

impl RngRange<f32> for RangeInclusive<f32> {
    fn start(&self) -> f32 {
        *self.start()
    }

    fn end(&self) -> f32 {
        *self.end()
    }
}

impl RngRange<u32> for Range<u32> {
    fn start(&self) -> u32 {
        self.start
    }

    fn end(&self) -> u32 {
        self.end - 1
    }
}

impl RngRange<u32> for RangeInclusive<u32> {
    fn start(&self) -> u32 {
        *self.start()
    }

    fn end(&self) -> u32 {
        *self.end()
    }
}
