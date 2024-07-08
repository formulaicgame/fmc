#![feature(portable_simd)]
#![allow(private_bounds)]

use std::simd::prelude::*;

use multiversion::{multiversion, target::selected_target};

mod abs;
mod add;
mod clamp;
mod constant;
mod fbm;
mod gradient;
mod lerp;
mod min_and_max;
mod mul;
mod noise_tree;
mod perlin;
mod range;
mod simplex;
mod square;

// TODO: Make a cargo feature "f64", makes it compile to f64 instead of f32
//if cfg(f64)
//type Float = f64;
//type Int = i64;
//else
//type Float = f32;
//type Int = i32;

// TODO: Find some way to make this Copy? You often use the same noise many places and clone makes
// it noisy.
#[derive(Clone, Debug)]
pub struct Noise {
    settings: NoiseSettings,
}

impl Noise {
    pub fn simplex(frequency: f32, seed: i32) -> Self {
        return Self {
            settings: NoiseSettings::Simplex {
                seed,
                frequency_x: frequency,
                frequency_y: frequency,
                frequency_z: frequency,
            },
        };
    }

    pub fn perlin(frequency: f32, seed: i32) -> Self {
        return Self {
            settings: NoiseSettings::Perlin {
                seed,
                frequency_x: frequency,
                frequency_y: frequency,
                frequency_z: frequency,
            },
        };
    }

    pub fn constant(value: f32) -> Self {
        return Self {
            settings: NoiseSettings::Constant { value },
        };
    }

    /// Set the frequency of the base noise.
    pub fn with_frequency(mut self, x: f32, y: f32, z: f32) -> Self {
        match &mut self.settings {
            NoiseSettings::Simplex {
                frequency_x,
                frequency_y,
                frequency_z,
                ..
            } => {
                *frequency_x = x;
                *frequency_y = y;
                *frequency_z = z;
            }
            NoiseSettings::Perlin {
                frequency_x,
                frequency_y,
                frequency_z,
                ..
            } => {
                *frequency_x = x;
                *frequency_y = y;
                *frequency_z = z;
            }
            _ => {
                // TODO: Recursively call and change the frequency for all noises?
                panic!("Frequency can only be changed when no other steps have been added.")
            }
        }
        self
    }

    /// Fractal Brownian Motion (layered noise)
    pub fn fbm(mut self, octaves: u32, gain: f32, lacunarity: f32) -> Self {
        let mut amp = gain;
        let mut scale = 1.0;

        for _ in 1..octaves {
            scale += amp;
            amp *= gain;
        }
        scale = 1.0 / scale;

        self.settings = NoiseSettings::Fbm {
            octaves,
            gain,
            lacunarity,
            scale,
            source: Box::new(self.settings),
        };
        self
    }

    /// Convert the noise to absolute values.
    pub fn abs(mut self) -> Self {
        self.settings = NoiseSettings::Abs {
            source: Box::new(self.settings),
        };
        self
    }

    /// Add two noises together, the result is not normalized.
    pub fn add(mut self, other: Self) -> Self {
        self.settings = NoiseSettings::AddNoise {
            left: Box::new(self.settings),
            right: Box::new(other.settings),
        };
        self
    }

    // TODO: Remove and replace with noise::constant, same for mul_value
    /// Add a value to the noise
    pub fn add_value(mut self, value: f32) -> Self {
        self.settings = NoiseSettings::AddValue {
            value,
            source: Box::new(self.settings),
        };
        self
    }

    /// Clamp the noise values between min and max
    pub fn clamp(mut self, min: f32, max: f32) -> Self {
        self.settings = NoiseSettings::Clamp {
            min,
            max,
            source: Box::new(self.settings),
        };
        self
    }

    /// Take the max value between the two noises
    pub fn max(mut self, other: Self) -> Self {
        self.settings = NoiseSettings::Max {
            left: Box::new(self.settings),
            right: Box::new(other.settings),
        };
        self
    }

    /// Take the min value between the two noises
    pub fn min(mut self, other: Self) -> Self {
        self.settings = NoiseSettings::Min {
            left: Box::new(self.settings),
            right: Box::new(other.settings),
        };
        self
    }

    // TODO: Convert to just 'mul' and take a noise
    /// Multiply the noise by a value.
    pub fn mul_value(mut self, value: f32) -> Self {
        self.settings = NoiseSettings::MulValue {
            value,
            source: Box::new(self.settings),
        };
        self
    }

    pub fn lerp(mut self, high: Self, low: Self) -> Self {
        self.settings = NoiseSettings::Lerp {
            selector_source: Box::new(self.settings),
            high_source: Box::new(high.settings),
            low_source: Box::new(low.settings),
        };
        self
    }

    pub fn range(mut self, high: f32, low: f32, high_noise: Self, low_noise: Self) -> Self {
        self.settings = NoiseSettings::Range {
            high,
            low,
            selector_source: Box::new(self.settings),
            high_source: Box::new(high_noise.settings),
            low_source: Box::new(low_noise.settings),
        };
        self
    }

    pub fn square(mut self) -> Self {
        self.settings = NoiseSettings::Square {
            source: Box::new(self.settings),
        };
        self
    }

    pub fn generate_1d(&self, x: f32, width: usize) -> (Vec<f32>, f32, f32) {
        generate_1d(self, x, width)
    }

    pub fn generate_2d(&self, x: f32, y: f32, width: usize, height: usize) -> (Vec<f32>, f32, f32) {
        generate_2d(self, x, y, width, height)
    }

    pub fn generate_3d(
        &self,
        x: f32,
        y: f32,
        z: f32,
        width: usize,
        height: usize,
        depth: usize,
    ) -> (Vec<f32>, f32, f32) {
        generate_3d(self, x, y, z, width, height, depth)
    }
}

#[derive(Clone, Debug)]
enum NoiseSettings {
    Simplex {
        seed: i32,
        frequency_x: f32,
        frequency_y: f32,
        frequency_z: f32,
    },
    Perlin {
        seed: i32,
        frequency_x: f32,
        frequency_y: f32,
        frequency_z: f32,
    },
    Constant {
        value: f32,
    },
    Fbm {
        /// Total number of octaves
        /// The number of octaves control the amount of detail in the noise function.
        /// Adding more octaves increases the detail, with the drawback of increasing the calculation time.
        octaves: u32,
        /// Gain is a multiplier on the amplitude of each successive octave.
        /// i.e. A gain of 2.0 will cause each octave to be twice as impactful on the result as the
        /// previous one.
        gain: f32,
        /// Lacunarity is multiplied by the frequency for each successive octave.
        /// i.e. a value of 2.0 will cause each octave to have double the frequency of the previous one.
        lacunarity: f32,
        // Automatically derived scaling factor.
        scale: f32,
        source: Box<NoiseSettings>,
    },
    Abs {
        source: Box<NoiseSettings>,
    },
    AddNoise {
        left: Box<NoiseSettings>,
        right: Box<NoiseSettings>,
    },
    AddValue {
        value: f32,
        source: Box<NoiseSettings>,
    },
    Clamp {
        min: f32,
        max: f32,
        source: Box<NoiseSettings>,
    },
    Lerp {
        selector_source: Box<NoiseSettings>,
        high_source: Box<NoiseSettings>,
        low_source: Box<NoiseSettings>,
    },
    Max {
        left: Box<NoiseSettings>,
        right: Box<NoiseSettings>,
    },
    Min {
        left: Box<NoiseSettings>,
        right: Box<NoiseSettings>,
    },
    MulValue {
        value: f32,
        source: Box<NoiseSettings>,
    },
    Range {
        high: f32,
        low: f32,
        selector_source: Box<NoiseSettings>,
        high_source: Box<NoiseSettings>,
        low_source: Box<NoiseSettings>,
    },
    Square {
        source: Box<NoiseSettings>,
    },
}

#[multiversion(targets = "simd")]
fn generate_1d(noise: &Noise, x: f32, width: usize) -> (Vec<f32>, f32, f32) {
    const N: usize = if let Some(size) = selected_target!().suggested_simd_width::<f32>() {
        size
    } else {
        1
    };

    let tree = noise_tree::NoiseTree::<N>::new(noise);

    let start_x = x;

    let mut min_s = Simd::splat(f32::MAX);
    let mut max_s = Simd::splat(f32::MIN);
    let mut min = f32::MAX;
    let mut max = f32::MIN;

    let mut result = Vec::with_capacity(width);
    unsafe {
        result.set_len(width);
    }
    let vector_width = N;
    let remainder = width % vector_width;
    let mut x_arr = Vec::with_capacity(vector_width);
    unsafe {
        x_arr.set_len(vector_width);
    }
    for i in (0..vector_width).rev() {
        x_arr[i] = start_x + i as f32;
    }

    let mut i = 0;
    let mut x = Simd::from_slice(&x_arr);
    for _ in 0..width / vector_width {
        let f = unsafe { (tree.nodes[0].function_1d)(&tree, &tree.nodes[0], x) };
        max_s = max_s.simd_max(f);
        min_s = min_s.simd_min(f);
        f.copy_to_slice(&mut result[i..]);
        i += vector_width;
        x += Simd::splat(vector_width as f32);
    }
    if remainder != 0 {
        let f = unsafe { (tree.nodes[0].function_1d)(&tree, &tree.nodes[0], x) };
        for j in 0..remainder {
            let n = f[j];
            unsafe {
                *result.get_unchecked_mut(i) = n;
            }
            if n < min {
                min = n;
            }
            if n > max {
                max = n;
            }
            i += 1;
        }
    }
    for i in 0..vector_width {
        if min_s[i] < min {
            min = min_s[i];
        }
        if max_s[i] > max {
            max = max_s[i];
        }
    }
    (result, min, max)
}

#[multiversion(targets = "simd")]
fn generate_2d(noise: &Noise, x: f32, y: f32, width: usize, height: usize) -> (Vec<f32>, f32, f32) {
    const N: usize = if let Some(size) = selected_target!().suggested_simd_width::<f32>() {
        size
    } else {
        1
    };

    let tree = noise_tree::NoiseTree::<N>::new(noise);
    let start_x = y;
    let start_y = x;

    let mut min_s = Simd::splat(f32::MAX);
    let mut max_s = Simd::splat(f32::MIN);
    let mut min = f32::MAX;
    let mut max = f32::MIN;

    let mut result = Vec::with_capacity(width * height);
    unsafe {
        result.set_len(width * height);
    }
    let mut y = Simd::splat(start_y);
    let mut i = 0;
    let vector_width = N;
    let remainder = width % vector_width;
    let mut x_arr = Vec::with_capacity(vector_width);
    unsafe {
        x_arr.set_len(vector_width);
    }
    for i in (0..vector_width).rev() {
        x_arr[i] = start_x + i as f32;
    }
    for _ in 0..height {
        let mut x = Simd::from_slice(&x_arr);
        for _ in 0..width / vector_width {
            let f = unsafe { (tree.nodes[0].function_2d)(&tree, &tree.nodes[0], x, y) };
            max_s = max_s.simd_max(f);
            min_s = min_s.simd_min(f);
            f.copy_to_slice(&mut result[i..]);
            i += vector_width;
            x += Simd::splat(vector_width as f32);
        }
        if remainder != 0 {
            let f = unsafe { (tree.nodes[0].function_2d)(&tree, &tree.nodes[0], x, y) };
            for j in 0..remainder {
                let n = f[j];
                unsafe {
                    *result.get_unchecked_mut(i) = n;
                }
                if n < min {
                    min = n;
                }
                if n > max {
                    max = n;
                }
                i += 1;
            }
        }
        y += Simd::splat(1.0);
    }
    for i in 0..vector_width {
        if min_s[i] < min {
            min = min_s[i];
        }
        if max_s[i] > max {
            max = max_s[i];
        }
    }
    (result, min, max)
}

#[multiversion(targets = "simd")]
fn generate_3d(
    noise: &Noise,
    x: f32,
    y: f32,
    z: f32,
    width: usize,
    height: usize,
    depth: usize,
) -> (Vec<f32>, f32, f32) {
    const N: usize = if let Some(size) = selected_target!().suggested_simd_width::<f32>() {
        size
    } else {
        1
    };
    let tree = noise_tree::NoiseTree::<N>::new(noise);

    let start_x = x;
    let start_y = y;
    let start_z = z;

    let mut min_s = Simd::splat(f32::MAX);
    let mut max_s = Simd::splat(f32::MIN);
    let mut min = f32::MAX;
    let mut max = f32::MIN;

    let mut result = Vec::with_capacity(width * height * depth);
    unsafe {
        result.set_len(width * height * depth);
    }
    let mut i = 0;
    let vector_width = N;
    let remainder = height % vector_width;
    let mut y_arr = Vec::with_capacity(vector_width);
    unsafe {
        y_arr.set_len(vector_width);
    }
    for i in (0..vector_width).rev() {
        y_arr[i] = start_y + i as f32;
    }

    // TODO: This loop in loop system is maybe not good? Try a flat design where "overflowing"
    // values of the first axis is transfered to the second, and same for second to third every
    // iteration.
    let mut x = Simd::splat(start_x);
    for _ in 0..width {
        let mut z = Simd::splat(start_z);
        for _ in 0..depth {
            let mut y = Simd::from_slice(&y_arr);
            for _ in 0..height / vector_width {
                let f = unsafe { (tree.nodes[0].function_3d)(&tree, &tree.nodes[0], x, y, z) };
                max_s = max_s.simd_max(f);
                min_s = min_s.simd_min(f);
                f.copy_to_slice(&mut result[i..]);
                i += vector_width;
                y = y + Simd::splat(vector_width as f32);
            }
            if remainder != 0 {
                let f = unsafe { (tree.nodes[0].function_3d)(&tree, &tree.nodes[0], x, y, z) };
                for j in 0..remainder {
                    let n = f[j];
                    unsafe {
                        *result.get_unchecked_mut(i) = n;
                    }
                    if n < min {
                        min = n;
                    }
                    if n > max {
                        max = n;
                    }
                    i += 1;
                }
            }
            z = z + Simd::splat(1.0);
        }
        x = x + Simd::splat(1.0);
    }
    for i in 0..vector_width {
        if min_s[i] < min {
            min = min_s[i];
        }
        if max_s[i] > max {
            max = max_s[i];
        }
    }
    (result, min, max)
}
