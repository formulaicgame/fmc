use std::simd::{LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn constant_1d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    _x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Constant { value } = &node.settings else {
        unreachable!()
    };

    return Simd::splat(*value);
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn constant_2d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    _x: Simd<f32, N>,
    _y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Constant { value } = &node.settings else {
        unreachable!()
    };

    return Simd::splat(*value);
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn constant_3d<const N: usize>(
    _tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    _x: Simd<f32, N>,
    _y: Simd<f32, N>,
    _z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Constant { value } = &node.settings else {
        unreachable!()
    };

    return Simd::splat(*value);
}
