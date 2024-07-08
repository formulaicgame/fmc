use std::simd::prelude::*;
use std::simd::{LaneCount, StdFloat, SupportedLaneCount};

use multiversion::multiversion;

use crate::gradient::grad3d_dot;
use crate::gradient::hash2d;
use crate::gradient::hash3d;
use crate::gradient::{grad1, grad2};
use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

pub const X_PRIME: i32 = 501125321;
pub const Y_PRIME: i32 = 1136930381;
pub const Z_PRIME: i32 = 1720413743;

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn perlin_2d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    mut x: Simd<f32, N>,
    mut y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Perlin {
        seed,
        frequency_x,
        frequency_z,
        ..
    } = node.settings
    else {
        unreachable!()
    };

    let seed = Simd::splat(seed);

    x *= Simd::splat(frequency_x);
    y *= Simd::splat(frequency_z);

    let mut xs = x.floor();
    let mut ys = y.floor();

    // NOTE: See simplex for unsafe
    let x0 = unsafe { xs.to_int_unchecked() * Simd::splat(X_PRIME) };
    let y0 = unsafe { ys.to_int_unchecked() * Simd::splat(Y_PRIME) };
    let x1 = x0 + Simd::splat(X_PRIME);
    let y1 = y0 + Simd::splat(Y_PRIME);

    let xf0 = x - xs;
    let yf0 = y - ys;

    let xf1 = xf0 - Simd::splat(1.0);
    let yf1 = yf0 - Simd::splat(1.0);

    xs = interpolate_quintic(xf0);
    ys = interpolate_quintic(yf0);

    return Simd::splat(0.579106986522674560546875)
        * lerp(
            lerp(
                grad2(hash2d(seed, x0, y0), xf0, yf0),
                grad2(hash2d(seed, x1, y0), xf1, yf0),
                xs,
            ),
            lerp(
                grad2(hash2d(seed, x0, y1), xf0, yf1),
                grad2(hash2d(seed, x1, y1), xf1, yf1),
                xs,
            ),
            ys,
        );
}
#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn perlin_3d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    mut x: Simd<f32, N>,
    mut y: Simd<f32, N>,
    mut z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Perlin {
        seed,
        frequency_x,
        frequency_y,
        frequency_z,
    } = node.settings
    else {
        unreachable!()
    };

    let seed = Simd::splat(seed);

    x *= Simd::splat(frequency_x);
    y *= Simd::splat(frequency_y);
    z *= Simd::splat(frequency_z);

    let mut xs = x.floor();
    let mut ys = y.floor();
    let mut zs = z.floor();

    let x0 = unsafe { xs.to_int_unchecked() * Simd::splat(X_PRIME) };
    let y0 = unsafe { ys.to_int_unchecked() * Simd::splat(Y_PRIME) };
    let z0 = unsafe { zs.to_int_unchecked() * Simd::splat(Z_PRIME) };
    let x1 = x0 + Simd::splat(X_PRIME);
    let y1 = y0 + Simd::splat(Y_PRIME);
    let z1 = z0 + Simd::splat(Z_PRIME);

    let xf0 = x - xs;
    let yf0 = y - ys;
    let zf0 = z - zs;

    let xf1 = xf0 - Simd::splat(1.0);
    let yf1 = yf0 - Simd::splat(1.0);
    let zf1 = zf0 - Simd::splat(1.0);

    xs = interpolate_quintic(xf0);
    ys = interpolate_quintic(yf0);
    zs = interpolate_quintic(zf0);

    return Simd::splat(0.964921414852142333984375)
        * lerp(
            lerp(
                lerp(
                    grad3d_dot(hash3d(seed, x0, y0, z0), xf0, yf0, zf0),
                    grad3d_dot(hash3d(seed, x1, y0, z0), xf1, yf0, zf0),
                    xs,
                ),
                lerp(
                    grad3d_dot(hash3d(seed, x0, y1, z0), xf0, yf1, zf0),
                    grad3d_dot(hash3d(seed, x1, y1, z0), xf1, yf1, zf0),
                    xs,
                ),
                ys,
            ),
            lerp(
                lerp(
                    grad3d_dot(hash3d(seed, x0, y0, z1), xf0, yf0, zf1),
                    grad3d_dot(hash3d(seed, x1, y0, z1), xf1, yf0, zf1),
                    xs,
                ),
                lerp(
                    grad3d_dot(hash3d(seed, x0, y1, z1), xf0, yf1, zf1),
                    grad3d_dot(hash3d(seed, x1, y1, z1), xf1, yf1, zf1),
                    xs,
                ),
                ys,
            ),
            zs,
        );
}

fn lerp<const N: usize>(a: Simd<f32, N>, b: Simd<f32, N>, t: Simd<f32, N>) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    return t.mul_add(b - a, a);
}

fn interpolate_quintic<const N: usize>(v: Simd<f32, N>) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    return v
        * v
        * v
        * v.mul_add(
            v.mul_add(Simd::splat(6.0), Simd::splat(-15.0)),
            Simd::splat(10.0),
        );
}
