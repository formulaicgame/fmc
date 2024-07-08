use std::simd::{LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_1d)(tree, &left_node, x)
            + (right_node.function_1d)(tree, &right_node, x);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_2d)(tree, &left_node, x, y)
            + (right_node.function_2d)(tree, &right_node, x, y);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_3d)(tree, &left_node, x, y, z)
            + (right_node.function_3d)(tree, &right_node, x, y, z);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_value_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_1d)(tree, &source, x) + Simd::splat(*value);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_value_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_2d)(tree, &source, x, y) + Simd::splat(*value);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn add_value_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::AddValue { value, source } = &node.settings else {
        unreachable!()
    };

    let source = &tree.nodes[*source];
    unsafe {
        return (source.function_3d)(tree, &source, x, y, z) + Simd::splat(*value);
    }
}
