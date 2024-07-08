use std::simd::{LaneCount, Simd, StdFloat, SupportedLaneCount};

use multiversion::multiversion;

use crate::noise_tree::{NoiseNode, NoiseNodeSettings, NoiseTree};

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn lerp_1d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Lerp {
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];
    unsafe {
        let high = (high_node.function_1d)(tree, &high_node, x);
        let low = (low_node.function_1d)(tree, &low_node, x);
        // This is just a special proprety of the -1..1 range. It's shifted up to be 0..1
        let interpolation =
            (selector.function_1d)(tree, &selector, x).mul_add(Simd::splat(0.5), Simd::splat(1.0));
        return (high - low).mul_add(interpolation, low);
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn lerp_2d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Lerp {
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];
    unsafe {
        let range = (high_node.function_2d)(tree, &high_node, x, y)
            - (low_node.function_2d)(tree, &low_node, x, y);
        let interpolation =
            ((selector.function_2d)(tree, &selector, x, y) + Simd::splat(1.0)) * Simd::splat(0.5);
        return range * interpolation;
    }
}

#[multiversion(targets = "simd", dispatcher = "pointer")]
pub fn lerp_3d<const N: usize>(
    tree: &NoiseTree<N>,
    node: &NoiseNode<N>,
    x: Simd<f32, N>,
    y: Simd<f32, N>,
    z: Simd<f32, N>,
) -> Simd<f32, N>
where
    LaneCount<N>: SupportedLaneCount,
{
    let NoiseNodeSettings::Lerp {
        selector,
        low_source,
        high_source,
    } = &node.settings
    else {
        unreachable!()
    };

    let selector = &tree.nodes[*selector];
    let low_node = &tree.nodes[*low_source];
    let high_node = &tree.nodes[*high_source];
    unsafe {
        let low_noise = (low_node.function_3d)(tree, &low_node, x, y, z);
        let high_noise = (high_node.function_3d)(tree, &high_node, x, y, z);

        let interpolation = ((selector.function_3d)(tree, &selector, x, y, z) + Simd::splat(1.0))
            * Simd::splat(0.5);

        return (high_noise - low_noise).mul_add(interpolation, low_noise);
    }
}
