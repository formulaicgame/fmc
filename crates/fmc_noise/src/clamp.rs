use std::simd::{prelude::*, LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn clamp_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Clamp { min, max, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_1d)(tree, &source, x)
            .simd_clamp(Simd::splat(*min), Simd::splat(*max));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn clamp_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Clamp { min, max, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_2d)(tree, &source, x, y)
            .simd_clamp(Simd::splat(*min), Simd::splat(*max));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn clamp_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Clamp { min, max, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_3d)(tree, &source, x, y, z)
            .simd_clamp(Simd::splat(*min), Simd::splat(*max));
    }
}
