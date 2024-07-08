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

const PERM: [i32; 512] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180, 151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194,
    233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234,
    75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174,
    20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83,
    111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25,
    63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188,
    159, 86, 164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147,
    118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
    213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253,
    19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193,
    238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31,
    181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

// SAFETY: Have to use to_int_unchecked because https://github.com/rust-lang/portable-simd/issues/325

/// Samples 1-dimensional simplex noise
///
/// Produces a value -1 ≤ n ≤ 1.
#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn simplex_1d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    mut x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Simplex {
        seed, frequency_x, ..
    } = node.settings
    else {
        unreachable!()
    };
    let seed = Simd::splat(seed);
    let freq = Simd::splat(frequency_x);
    x *= freq;

    // Gradients are selected deterministically based on the whole part of `x`
    let ips = x.floor();
    // NOTE: Converting to int normally was very slow for some reason I don't remember. It called
    // out to libm? Note applies to perlin noise too.
    let mut i0: Simd<i32, N> = unsafe { ips.to_int_unchecked() };
    let i1 = (i0 + Simd::splat(1)) & Simd::splat(0xff);

    // the fractional part of x, i.e. the distance to the left gradient node. 0 ≤ x0 < 1.
    let x0 = x - ips;
    // signed distance to the right gradient node
    let x1 = x0 - Simd::splat(1.0);

    i0 = i0 & Simd::splat(0xff);
    let gi0 = Simd::gather_or_default(&PERM, i0.cast());
    let gi1 = Simd::gather_or_default(&PERM, i1.cast());

    // Compute the contribution from the first gradient
    let x20 = x0 * x0; // x^2_0
    let t0 = Simd::splat(1.0) - x20; // t_0
    let t20 = t0 * t0; // t^2_0
    let t40 = t20 * t20; // t^4_0
    let gx0 = grad1(seed, gi0);
    let n0 = t40 * gx0 * x0;
    // n0 = (1 - x0^2)^4 * x0 * grad

    // Compute the contribution from the second gradient
    let x21 = x1 * x1; // x^2_1
    let t1 = Simd::splat(1.0) - x21; // t_1
    let t21 = t1 * t1; // t^2_1
    let t41 = t21 * t21; // t^4_1
    let gx1 = grad1(seed, gi1);
    let n1 = t41 * gx1 * x1;

    // n0 + n1 =
    //    grad0 * x0 * (1 - x0^2)^4
    //  + grad1 * (x0 - 1) * (1 - (x0 - 1)^2)^4
    //
    // Assuming worst-case values for grad0 and grad1, we therefore need only determine the maximum of
    //
    // |x0 * (1 - x0^2)^4| + |(x0 - 1) * (1 - (x0 - 1)^2)^4|
    //
    // for 0 ≤ x0 < 1. This can be done by root-finding on the derivative, obtaining 81 / 256 when
    // x0 = 0.5, which we finally multiply by the maximum gradient to get the maximum value,
    // allowing us to scale into [-1, 1]
    const SCALE: f32 = 256.0 / (81.0 * 7.0);

    let value = (n0 + n1) * Simd::splat(SCALE);
    //let derivative = ((t20 * t0 * gx0 * x20 + t21 * t1 * gx1 * x21) * Simd::splat(-8.0)
    //    + t40 * gx0
    //    + t41 * gx1)
    //    * Simd::splat(SCALE);
    value
}

/// Samples 2-dimensional simplex noise
///
/// Produces a value -1 ≤ n ≤ 1.
#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn simplex_2d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    mut x: Simd<f32, N>,
    mut y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    const SQRT3: f32 = 1.7320508075688772935274463415059;
    const F2: f32 = 0.5 * (SQRT3 - 1.0);
    const G2: f32 = (3.0 - SQRT3) / 6.0;

    let NoiseNodeSettings::Simplex {
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

    let f = Simd::splat(F2) * (x + y);
    let mut x0 = (x + f).floor();
    let mut y0 = (y + f).floor();

    let i = unsafe { x0.to_int_unchecked() * Simd::splat(X_PRIME) };
    let j = unsafe { y0.to_int_unchecked() * Simd::splat(Y_PRIME) };

    let g = Simd::splat(G2) * (x0 + y0);
    x0 = x - (x0 - g);
    y0 = y - (y0 - g);

    let i1 = x0.simd_gt(y0);
    //j1 = ~i1; //NMasked funcs

    let x1 = i1.select(x0 - Simd::splat(1.0), x0) + Simd::splat(G2);
    let y1 = i1.select(y0, y0 - Simd::splat(1.0)) + Simd::splat(G2);

    let x2 = x0 + Simd::splat(G2 * 2.0 - 1.0);
    let y2 = y0 + Simd::splat(G2 * 2.0 - 1.0);

    let mut t0 = x0.mul_add(-x0, y0.mul_add(-y0, Simd::splat(0.5)));
    let mut t1 = x1.mul_add(-x1, y1.mul_add(-y1, Simd::splat(0.5)));
    let mut t2 = x2.mul_add(-x2, y2.mul_add(-y2, Simd::splat(0.5)));

    t0 = t0.simd_max(Simd::splat(0.0));
    t1 = t1.simd_max(Simd::splat(0.0));
    t2 = t2.simd_max(Simd::splat(0.0));

    t0 *= t0;
    t0 *= t0;
    t1 *= t1;
    t1 *= t1;
    t2 *= t2;
    t2 *= t2;

    let n0 = grad2(hash2d(seed, i, j), x0, y0);
    let j1 = i1.select(j, j + Simd::splat(Y_PRIME));
    let i1 = i1.select(i + Simd::splat(X_PRIME), i);
    let n1 = grad2(hash2d(seed, i1, j1), x1, y1);
    let i2 = i + Simd::splat(X_PRIME);
    let j2 = j + Simd::splat(Y_PRIME);
    let n2 = grad2(hash2d(seed, i2, j2), x2, y2);

    return Simd::splat(38.283687591552734375) * n0.mul_add(t0, n1.mul_add(t1, n2 * t2));
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn simplex_3d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    mut x: Simd<f32, N>,
    mut y: Simd<f32, N>,
    mut z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    const F3: f32 = 1.0 / 3.0;
    const G3: f32 = 1.0 / 2.0;

    let NoiseNodeSettings::Simplex {
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

    let s = Simd::splat(F3) * (x + y + z);
    x += s;
    y += s;
    z += s;

    let mut x0 = x.floor();
    let mut y0 = y.floor();
    let mut z0 = z.floor();
    let xi = x - x0;
    let yi = y - y0;
    let zi = z - z0;

    let i = unsafe { x0.to_int_unchecked() * Simd::splat(X_PRIME) };
    let j = unsafe { y0.to_int_unchecked() * Simd::splat(Y_PRIME) };
    let k = unsafe { z0.to_int_unchecked() * Simd::splat(Z_PRIME) };

    let x_ge_y = xi.simd_ge(yi);
    let y_ge_z = yi.simd_ge(zi);
    let x_ge_z = xi.simd_ge(zi);

    let g = Simd::splat(G3) * (xi + yi + zi);
    x0 = xi - g;
    y0 = yi - g;
    z0 = zi - g;

    let i1 = x_ge_y & x_ge_z;
    let j1 = y_ge_z & !x_ge_y;
    let k1 = !x_ge_z & !y_ge_z;

    let i2 = x_ge_y | x_ge_z;
    let j2 = !x_ge_y | y_ge_z;
    let k2 = x_ge_z & y_ge_z; //NMasked

    let x1 = i1.select(x0 - Simd::splat(1.0), x0) + Simd::splat(G3);
    let y1 = j1.select(y0 - Simd::splat(1.0), y0) + Simd::splat(G3);
    let z1 = k1.select(z0 - Simd::splat(1.0), z0) + Simd::splat(G3);
    let x2 = i2.select(x0 - Simd::splat(1.0), x0) + Simd::splat(G3 * 2.0);
    let y2 = j2.select(y0 - Simd::splat(1.0), y0) + Simd::splat(G3 * 2.0);
    let z2 = k2.select(z0, z0 - Simd::splat(1.0)) + Simd::splat(G3 * 2.0);
    let x3 = x0 + Simd::splat(G3 * 3.0 - 1.0);
    let y3 = y0 + Simd::splat(G3 * 3.0 - 1.0);
    let z3 = z0 + Simd::splat(G3 * 3.0 - 1.0);

    let mut t0 = x0.mul_add(-x0, y0.mul_add(-y0, z0.mul_add(-z0, Simd::splat(0.6))));
    let mut t1 = x1.mul_add(-x1, y1.mul_add(-y1, z1.mul_add(-z1, Simd::splat(0.6))));
    let mut t2 = x2.mul_add(-x2, y2.mul_add(-y2, z2.mul_add(-z2, Simd::splat(0.6))));
    let mut t3 = x3.mul_add(-x3, y3.mul_add(-y3, z3.mul_add(-z3, Simd::splat(0.6))));

    t0 = t0.simd_max(Simd::splat(0.0));
    t1 = t1.simd_max(Simd::splat(0.0));
    t2 = t2.simd_max(Simd::splat(0.0));
    t3 = t3.simd_max(Simd::splat(0.0));

    // Square twice
    t0 *= t0;
    t0 *= t0;
    t1 *= t1;
    t1 *= t1;
    t2 *= t2;
    t2 *= t2;
    t3 *= t3;
    t3 *= t3;

    let n0 = grad3d_dot(hash3d(seed, i, j, k), x0, y0, z0);
    let i1 = i1.select(i + Simd::splat(X_PRIME), i);
    let j1 = j1.select(j + Simd::splat(Y_PRIME), j);
    let k1 = k1.select(k + Simd::splat(Z_PRIME), k);
    let n1 = grad3d_dot(hash3d(seed, i1, j1, k1), x1, y1, z1);
    let i2 = i2.select(i + Simd::splat(X_PRIME), i);
    let j2 = j2.select(j + Simd::splat(Y_PRIME), j);
    let k2 = k2.select(k, k + Simd::splat(Z_PRIME));
    let n2 = grad3d_dot(hash3d(seed, i2, j2, k2), x2, y2, z2);
    let i3 = i + Simd::splat(X_PRIME);
    let j3 = j + Simd::splat(Y_PRIME);
    let k3 = k + Simd::splat(Z_PRIME);
    let n3 = grad3d_dot(hash3d(seed, i3, j3, k3), x3, y3, z3);

    return Simd::splat(32.69428253173828125)
        * n0.mul_add(t0, n1.mul_add(t1, n2.mul_add(t2, n3 * t3)));
}
