use std::simd::{prelude::*, LaneCount, Simd, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn max_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MaxNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_1d)(tree, &left_node, x).simd_max((right_node.function_1d)(
            tree,
            &right_node,
            x,
        ));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn max_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MaxNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_2d)(tree, &left_node, x, y).simd_max((right_node.function_2d)(
            tree,
            &right_node,
            x,
            y,
        ));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn max_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MaxNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_3d)(tree, &left_node, x, y, z).simd_max((right_node
            .function_3d)(
            tree,
            &right_node,
            x,
            y,
            z,
        ));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn min_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MinNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_1d)(tree, &left_node, x).simd_min((right_node.function_1d)(
            tree,
            &right_node,
            x,
        ));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn min_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MinNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_2d)(tree, &left_node, x, y).simd_min((right_node.function_2d)(
            tree,
            &right_node,
            x,
            y,
        ));
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn min_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::MinNoise {
        left_source,
        right_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let left_node = &tree.nodes[*left_source];
    let right_node = &tree.nodes[*right_source];
    unsafe {
        return (left_node.function_3d)(tree, &left_node, x, y, z).simd_min((right_node
            .function_3d)(
            tree,
            &right_node,
            x,
            y,
            z,
        ));
    }
}
