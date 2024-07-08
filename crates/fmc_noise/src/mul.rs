use std::simd::{LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn mul_value_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MulValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_1d)(tree, &source, x) * Simd::splat(*value);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn mul_value_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MulValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_2d)(tree, &source, x, y) * Simd::splat(*value);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn mul_value_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MulValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_3d)(tree, &source, x, y, z) * Simd::splat(*value);
    }
}
