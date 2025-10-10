#[derive(Default, Debug, Clone)]
pub struct Rng {
    seed: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn next_usize(&mut self) -> usize {
        let seed = self.seed.wrapping_add(0x2d35_8dcc_aa6c_78a5);
        self.seed = seed;
        let t = u128::from(seed) * u128::from(seed ^ 0x8bb8_4b93_962e_acc9);
        return ((t as u64) ^ (t >> 64) as u64) as usize;
    }

    pub fn next_u32(&mut self) -> u32 {
        self.next_usize() as u32
    }

    pub fn next_i32(&mut self) -> i32 {
        self.next_usize() as i32
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

#[derive(Clone, Debug)]
pub struct UniformDistribution<T> {
    start: T,
    range: T,
}

impl<T: private::UniformType> UniformDistribution<T> {
    pub fn new(low: T, high: T) -> Self {
        assert!(low <= high);

        Self {
            start: low,
            range: high - low,
        }
    }

    pub fn sample(&self, rng: &mut Rng) -> T {
        T::sample(rng, self.start, self.range)
    }
}

mod private {
    use super::Rng;
    pub trait UniformType:
        PartialOrd + Copy + std::ops::Sub<Output = Self> + std::fmt::Debug
    {
        fn sample(rng: &mut Rng, start: Self, range: Self) -> Self;
    }

    impl UniformType for f32 {
        fn sample(rng: &mut Rng, start: Self, range: Self) -> Self {
            start + rng.next_f32() * range
        }
    }
    impl UniformType for u32 {
        fn sample(rng: &mut Rng, start: Self, range: Self) -> Self {
            start + rng.next_u32() % (range + 1)
        }
    }
    impl UniformType for i32 {
        fn sample(rng: &mut Rng, start: Self, range: Self) -> Self {
            start + rng.next_i32().abs() % (range + 1)
        }
    }
    impl UniformType for usize {
        fn sample(rng: &mut Rng, start: Self, range: Self) -> Self {
            start + rng.next_usize() % (range + 1)
        }
    }
}

#[derive(Clone)]
pub struct Bernoulli {
    probability: f32,
}

impl Bernoulli {
    pub fn new(probability: f32) -> Self {
        assert!(probability >= 0.0 && probability <= 1.0);

        Self { probability }
    }

    pub fn sample(&self, rng: &mut Rng) -> bool {
        rng.next_f32() < self.probability
    }
}

pub struct Choose<'a, T> {
    slice: &'a [T],
    range: UniformDistribution<usize>,
}

impl<'a, T> Choose<'a, T> {
    pub fn new(slice: &'a [T]) -> Self {
        assert!(slice.len() > 0);
        Self {
            slice,
            range: UniformDistribution::new(0, slice.len() - 1),
        }
    }

    pub fn sample(&self, rng: &mut Rng) -> &'a T {
        unsafe { self.slice.get_unchecked(self.range.sample(rng)) }
    }

    pub fn iter(&'a self, rng: &'a mut Rng) -> ChooseIter<'a, T> {
        ChooseIter { choose: self, rng }
    }
}

pub struct ChooseIter<'a, T> {
    choose: &'a Choose<'a, T>,
    rng: &'a mut Rng,
}

impl<'a, T> Iterator for ChooseIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            Some(
                self.choose
                    .slice
                    .get_unchecked(self.choose.range.sample(self.rng)),
            )
        }
    }
}

#[derive(Clone, Debug)]
pub struct WeightedIndex<T: private::UniformType> {
    cumulative_weights: Vec<T>,
    weight_distribution: UniformDistribution<T>,
}

impl<T: private::UniformType> WeightedIndex<T> {
    pub fn new<'a>(weights: impl IntoIterator<Item = &'a T>) -> Result<WeightedIndex<T>, ()>
    where
        T: core::ops::AddAssign + Clone + Default + 'a,
    {
        let mut total_weight = T::default();
        let iter = weights.into_iter();
        let mut cumulative_weights = Vec::<T>::with_capacity(iter.size_hint().0);

        for w in iter {
            if !(*w >= T::default()) {
                return Err(());
            }
            total_weight += *w;
            cumulative_weights.push(total_weight);
        }

        Ok(WeightedIndex {
            cumulative_weights,
            weight_distribution: UniformDistribution::new(T::default(), total_weight),
        })
    }

    pub fn sample(&self, rng: &mut Rng) -> usize {
        use ::core::cmp::Ordering;
        let chosen_weight = self.weight_distribution.sample(rng);
        // Find the first item which has a weight *higher* than the chosen weight.
        self.cumulative_weights
            .binary_search_by(|w| {
                if *w <= chosen_weight {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            })
            .unwrap_err()
    }
}
