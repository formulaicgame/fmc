use std::simd::{prelude::*, LaneCount, Simd, StdFloat, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn range_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Range {
        high,
        low,
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let high = Simd::splat(*high);
    let low = Simd::splat(*low);
    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];

    unsafe {
        let selection_noise = (selector.function_1d)(tree, &selector, x);
        let low_noise = (low_node.function_1d)(tree, &low_node, x);
        let high_noise = (high_node.function_1d)(tree, &high_node, x);

        let high_clipped = selection_noise.simd_gt(high);
        let low_clipped = selection_noise.simd_lt(low);

        let mut interpolation = (selection_noise - low) / (high - low);
        interpolation = (high_noise - low_noise).mul_add(interpolation, low_noise);

        let mut result = high_clipped.select(high_noise, interpolation);
        result = low_clipped.select(low_noise, result);
        return result;
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn range_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Range {
        high,
        low,
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let high = Simd::splat(*high);
    let low = Simd::splat(*low);
    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];

    unsafe {
        let selection_noise = (selector.function_2d)(tree, &selector, x, y);
        let low_noise = (low_node.function_2d)(tree, &low_node, x, y);
        let high_noise = (high_node.function_2d)(tree, &high_node, x, y);

        let high_clipped = selection_noise.simd_gt(high);
        let low_clipped = selection_noise.simd_lt(low);

        let mut interpolation = (selection_noise - low) / (high - low);
        interpolation = (high_noise - low_noise).mul_add(interpolation, low_noise);

        let mut result = high_clipped.select(high_noise, interpolation);
        result = low_clipped.select(low_noise, result);
        return result;
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn range_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Range {
        high,
        low,
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let high = Simd::splat(*high);
    let low = Simd::splat(*low);
    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];

    unsafe {
        let selection_noise = (selector.function_3d)(tree, &selector, x, y, z);
        let low_noise = (low_node.function_3d)(tree, &low_node, x, y, z);
        let high_noise = (high_node.function_3d)(tree, &high_node, x, y, z);

        let high_clipped = selection_noise.simd_gt(high);
        let low_clipped = selection_noise.simd_lt(low);

        let mut interpolation = (selection_noise - low) / (high - low);
        interpolation = (high_noise - low_noise).mul_add(interpolation, low_noise);

        let mut result = high_clipped.select(high_noise, interpolation);
        result = low_clipped.select(low_noise, result);
        return result;
    }
}
