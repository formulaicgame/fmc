use std::simd::{LaneCount, Simd, num::SimdFloat, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn abs_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Abs { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_1d)(tree, &source, x).abs();
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn abs_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Abs { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_2d)(tree, &source, x, y).abs();
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn abs_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Abs { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_3d)(tree, &source, x, y, z).abs();
    }
}
