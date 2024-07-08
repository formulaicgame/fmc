use std::simd::{LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn square_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Square { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        let source_result = (source.function_1d)(tree, &source, x);
        return source_result * source_result;
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn square_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Square { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        let source_result = (source.function_2d)(tree, &source, x, y);
        return source_result * source_result;
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn square_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Square { source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        let source_result = (source.function_3d)(tree, &source, x, y, z);
        return source_result * source_result;
    }
}
