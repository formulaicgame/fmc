use std::simd::{prelude::*, LaneCount, StdFloat, SupportedLaneCount};

#[inline(always)]
pub fn grad1<const N: usize>(seed: Simd<i32, N>, hash: Simd<i32, N>) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let h = (seed ^ hash) & Simd::splat(15);
    let v = (h & Simd::splat(7)).cast();

    let h_and_8 = (h & Simd::splat(8)).simd_eq(Simd::splat(0));
    h_and_8.select(Simd::splat(0.0) - v, v)
}

#[inline(always)]
pub fn grad2<const N: usize>(
    hash: Simd<i32, N>,
    mut x: Simd<f32, N>,
    mut y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    const ROOT2: f32 = 1.4142135623730950488;
    // ( 1+R2, 1 ) ( -1-R2, 1 ) ( 1+R2, -1 ) ( -1-R2, -1 )
    // ( 1, 1+R2 ) ( 1, -1-R2 ) ( -1, 1+R2 ) ( -1, -1-R2 )

    let bit1: Simd<i32, N> = hash << Simd::splat(31);
    let bit2 = (hash >> Simd::splat(1)) << Simd::splat(31);

    // TODO: Implemented without knowing what the sign is supposed to be. Think it might need to be
    // negated.
    let bit4 = Mask::from_int((hash << Simd::splat(29)) >> Simd::splat(31));

    x = Simd::from_bits(x.to_bits() ^ bit1.cast::<u32>());
    y = Simd::from_bits(y.to_bits() ^ bit2.cast::<u32>());

    let a = bit4.select(y, x);
    let b = bit4.select(x, y);

    return Simd::splat(1.0 + ROOT2).mul_add(a, b);
}

/// Generates a random gradient vector from the origin towards the midpoint of an edge of a
/// double-unit cube and computes its dot product with [x, y, z]
#[inline(always)]
pub fn grad3d_dot<const N: usize>(
    hash: Simd<i32, N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let hasha13 = hash & Simd::splat(13);

    //if h < 8 then x, else y
    let u = hasha13.simd_lt(Simd::splat(8)).select(x, y);

    //if h < 2 then y else if h is 12 or 14 then x else z
    let mut v = hasha13.simd_eq(Simd::splat(12)).select(x, z);
    v = hasha13.simd_lt(Simd::splat(2)).select(y, v);

    //if h1 then -u else u
    //if h2 then -v else v
    let h1 = (hash << Simd::splat(31)).cast::<u32>();
    let h2 = ((hash & Simd::splat(2)) << Simd::splat(30)).cast::<u32>();
    //then add them
    let a = Simd::<f32, N>::from_bits(u.to_bits() ^ h1);
    let b = Simd::<f32, N>::from_bits(v.to_bits() ^ h2);
    return a + b;
    //return Simd::<f32, N>::from_bits(u.to_bits() ^ h1) + Simd::from_bits(v.to_bits() ^ h2);
}

#[inline(always)]
pub fn hash2d<const N: usize>(seed: Simd<i32, N>, i: Simd<i32, N>, j: Simd<i32, N>) -> Simd<i32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let mut hash = seed;
    hash ^= i;
    hash ^= j;

    hash *= Simd::splat(0x27d4eb2d);
    return (hash >> Simd::splat(15)) ^ hash;
}

#[inline(always)]
pub fn hash3d<const N: usize>(
    seed: Simd<i32, N>,
    i: Simd<i32, N>,
    j: Simd<i32, N>,
    k: Simd<i32, N>,
) -> Simd<i32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let mut hash = seed;
    hash ^= i;
    hash ^= j;
    hash ^= k;

    hash *= Simd::splat(0x27d4eb2d);
    return (hash >> Simd::splat(15)) ^ hash;
}
