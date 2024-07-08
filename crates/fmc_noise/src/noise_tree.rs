use std::simd::{LaneCount, Simd, SupportedLaneCount};

use crate::{Noise, NoiseSettings};

#[derive(Debug)]
pub struct NoiseTree<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub nodes: Vec<NoiseNode<N>>,
}

impl<const N: usize> NoiseTree<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub fn new(value: &Noise) -> Self {
        fn add_node<const N: usize>(nodes: &mut Vec<NoiseNode<N>>, settings: &NoiseSettings)
        where
            LaneCount<N>: SupportedLaneCount,
        {
            match settings {
                NoiseSettings::Simplex {
                    seed,
                    frequency_x,
                    frequency_y,
                    frequency_z,
                } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Simplex {
                            seed: *seed,
                            frequency_x: *frequency_x,
                            frequency_y: *frequency_y,
                            frequency_z: *frequency_z,
                        },
                        function_1d: crate::simplex::simplex_1d(),
                        function_2d: crate::simplex::simplex_2d(),
                        function_3d: crate::simplex::simplex_3d(),
                    });
                }
                NoiseSettings::Perlin {
                    seed,
                    frequency_x,
                    frequency_y,
                    frequency_z,
                } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Perlin {
                            seed: *seed,
                            frequency_x: *frequency_x,
                            frequency_y: *frequency_y,
                            frequency_z: *frequency_z,
                        },
                        function_1d: crate::simplex::simplex_1d(),
                        function_2d: crate::perlin::perlin_2d(),
                        function_3d: crate::perlin::perlin_3d(),
                    });
                }
                NoiseSettings::Constant { value } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Constant { value: *value },
                        function_1d: crate::constant::constant_1d(),
                        function_2d: crate::constant::constant_2d(),
                        function_3d: crate::constant::constant_3d(),
                    });
                }
                NoiseSettings::Fbm {
                    octaves,
                    gain,
                    lacunarity,
                    scale,
                    source,
                } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Fbm {
                            octaves: *octaves,
                            gain: *gain,
                            lacunarity: *lacunarity,
                            scale: *scale,
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::fbm::fbm_1d(),
                        function_2d: crate::fbm::fbm_2d(),
                        function_3d: crate::fbm::fbm_3d(),
                    });
                    add_node(nodes, source);
                }
                NoiseSettings::Abs { source } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Abs {
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::abs::abs_1d(),
                        function_2d: crate::abs::abs_2d(),
                        function_3d: crate::abs::abs_3d(),
                    });
                    add_node(nodes, source);
                }
                NoiseSettings::AddNoise { left, right } => {
                    // push a fake to reserve the index
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::AddNoise {
                            left_source: 0,
                            right_source: 0,
                        },
                        function_1d: crate::add::add_1d(),
                        function_2d: crate::add::add_2d(),
                        function_3d: crate::add::add_3d(),
                    });
                    let index = nodes.len() - 1;

                    let left_source = nodes.len();
                    add_node(nodes, left);
                    let right_source = nodes.len();
                    add_node(nodes, right);

                    nodes[index] = NoiseNode {
                        settings: NoiseNodeSettings::AddNoise {
                            left_source,
                            right_source,
                        },
                        function_1d: crate::add::add_1d(),
                        function_2d: crate::add::add_2d(),
                        function_3d: crate::add::add_3d(),
                    };
                }
                NoiseSettings::AddValue { value, source } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::AddValue {
                            value: *value,
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::add::add_value_1d(),
                        function_2d: crate::add::add_value_2d(),
                        function_3d: crate::add::add_value_3d(),
                    });
                    add_node(nodes, source);
                }
                NoiseSettings::Clamp { min, max, source } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Clamp {
                            min: *min,
                            max: *max,
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::clamp::clamp_1d(),
                        function_2d: crate::clamp::clamp_2d(),
                        function_3d: crate::clamp::clamp_3d(),
                    });
                    add_node(nodes, source);
                }
                NoiseSettings::Max { left, right } => {
                    // push a fake to reserve the index
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::MaxNoise {
                            left_source: 0,
                            right_source: 0,
                        },
                        function_1d: crate::min_and_max::max_1d(),
                        function_2d: crate::min_and_max::max_2d(),
                        function_3d: crate::min_and_max::max_3d(),
                    });
                    let index = nodes.len() - 1;

                    let left_source = nodes.len();
                    add_node(nodes, left);
                    let right_source = nodes.len();
                    add_node(nodes, right);

                    nodes[index] = NoiseNode {
                        settings: NoiseNodeSettings::MaxNoise {
                            left_source,
                            right_source,
                        },
                        function_1d: crate::min_and_max::max_1d(),
                        function_2d: crate::min_and_max::max_2d(),
                        function_3d: crate::min_and_max::max_3d(),
                    };
                }
                NoiseSettings::Min { left, right } => {
                    // push a fake to reserve the index
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::MinNoise {
                            left_source: 0,
                            right_source: 0,
                        },
                        function_1d: crate::min_and_max::min_1d(),
                        function_2d: crate::min_and_max::min_2d(),
                        function_3d: crate::min_and_max::min_3d(),
                    });
                    let index = nodes.len() - 1;

                    let left_source = nodes.len();
                    add_node(nodes, left);
                    let right_source = nodes.len();
                    add_node(nodes, right);

                    nodes[index] = NoiseNode {
                        settings: NoiseNodeSettings::MinNoise {
                            left_source,
                            right_source,
                        },
                        function_1d: crate::min_and_max::min_1d(),
                        function_2d: crate::min_and_max::min_2d(),
                        function_3d: crate::min_and_max::min_3d(),
                    };
                }
                NoiseSettings::MulValue { value, source } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::MulValue {
                            value: *value,
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::mul::mul_value_1d(),
                        function_2d: crate::mul::mul_value_2d(),
                        function_3d: crate::mul::mul_value_3d(),
                    });
                    add_node(nodes, source);
                }
                NoiseSettings::Lerp {
                    selector_source: selector,
                    low_source: low,
                    high_source: high,
                } => {
                    // push a fake to reserve the index
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Lerp {
                            selector: 0,
                            low_source: 0,
                            high_source: 0,
                        },
                        function_1d: crate::lerp::lerp_1d(),
                        function_2d: crate::lerp::lerp_2d(),
                        function_3d: crate::lerp::lerp_3d(),
                    });
                    let index = nodes.len() - 1;

                    let select_idx = nodes.len();
                    add_node(nodes, selector);
                    let low_idx = nodes.len();
                    add_node(nodes, low);
                    let high_idx = nodes.len();
                    add_node(nodes, high);

                    nodes[index] = NoiseNode {
                        settings: NoiseNodeSettings::Lerp {
                            selector: select_idx,
                            low_source: low_idx,
                            high_source: high_idx,
                        },
                        function_1d: crate::lerp::lerp_1d(),
                        function_2d: crate::lerp::lerp_2d(),
                        function_3d: crate::lerp::lerp_3d(),
                    };
                }
                NoiseSettings::Range {
                    high,
                    low,
                    selector_source,
                    high_source,
                    low_source,
                } => {
                    // push a fake to reserve the index
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Range {
                            low: 0.0,
                            high: 0.0,
                            selector: 0,
                            low_source: 0,
                            high_source: 0,
                        },
                        function_1d: crate::range::range_1d(),
                        function_2d: crate::range::range_2d(),
                        function_3d: crate::range::range_3d(),
                    });
                    let index = nodes.len() - 1;

                    let select_idx = nodes.len();
                    add_node(nodes, selector_source);
                    let low_idx = nodes.len();
                    add_node(nodes, low_source);
                    let high_idx = nodes.len();
                    add_node(nodes, high_source);

                    nodes[index] = NoiseNode {
                        settings: NoiseNodeSettings::Range {
                            low: *low,
                            high: *high,
                            selector: select_idx,
                            low_source: low_idx,
                            high_source: high_idx,
                        },
                        function_1d: crate::range::range_1d(),
                        function_2d: crate::range::range_2d(),
                        function_3d: crate::range::range_3d(),
                    };
                }
                NoiseSettings::Square { source } => {
                    nodes.push(NoiseNode {
                        settings: NoiseNodeSettings::Square {
                            source: nodes.len() + 1,
                        },
                        function_1d: crate::square::square_1d(),
                        function_2d: crate::square::square_2d(),
                        function_3d: crate::square::square_3d(),
                    });
                    add_node(nodes, source);
                }
            };
        }
        let mut nodes = Vec::with_capacity(8);
        add_node(&mut nodes, &value.settings);

        return Self { nodes };
    }
}

#[derive(Debug)]
pub(crate) enum NoiseNodeSettings {
    Simplex {
        seed: i32,
        frequency_x: f32,
        frequency_y: f32,
        frequency_z: f32,
    },
    Perlin {
        seed: i32,
        frequency_x: f32,
        frequency_y: f32,
        frequency_z: f32,
    },
    Constant {
        value: f32,
    },
    Fbm {
        octaves: u32,
        gain: f32,
        lacunarity: f32,
        scale: f32,
        source: usize,
    },
    Abs {
        source: usize,
    },
    AddNoise {
        left_source: usize,
        right_source: usize,
    },
    AddValue {
        value: f32,
        source: usize,
    },
    Clamp {
        min: f32,
        max: f32,
        source: usize,
    },
    MaxNoise {
        left_source: usize,
        right_source: usize,
    },
    MinNoise {
        left_source: usize,
        right_source: usize,
    },
    MulValue {
        value: f32,
        source: usize,
    },
    Lerp {
        selector: usize,
        low_source: usize,
        high_source: usize,
    },
    Range {
        low: f32,
        high: f32,
        selector: usize,
        low_source: usize,
        high_source: usize,
    },
    Square {
        source: usize,
    },
}

#[derive(Debug)]
pub(crate) struct NoiseNode<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub settings: NoiseNodeSettings,
    pub function_1d:
        unsafe fn(noise_tree: &NoiseTree<N>, node: &NoiseNode<N>, x: Simd<f32, N>) -> Simd<f32, N>,
    pub function_2d: unsafe fn(
        noise_tree: &NoiseTree<N>,
        node: &NoiseNode<N>,
        x: Simd<f32, N>,
        y: Simd<f32, N>,
    ) -> Simd<f32, N>,
    pub function_3d: unsafe fn(
        noise_tree: &NoiseTree<N>,
        node: &NoiseNode<N>,
        x: Simd<f32, N>,
        y: Simd<f32, N>,
        z: Simd<f32, N>,
    ) -> Simd<f32, N>,
}

// TODO: Nesting pointers has the same performance on my system with the benefit of reducing code
// complexity. Feels like it will suddenly break down if the cpu doesn't have good branch
// prediction. Test on old computer.
//impl<const N: usize> NoiseNode<N>
//where
//    LaneCount<N>: SupportedLaneCount
//{
//    fn from(value: &NoiseSettings) -> Self {
//        match value {
//            NoiseSettings::Simplex { frequency } => {
//                NoiseNode {
//                    settings: NoiseNodeSettings::Simplex { frequency: *frequency },
//                    generate_3d: crate::simplex::simplex_3d()
//                }
//            },
//            NoiseSettings::Fbm { settings, source } => {
//                NoiseNode {
//                    settings: NoiseNodeSettings::Fbm {
//                        settings: settings.clone(),
//                        source: Box::new(Self::from(source.as_ref()))
//                    },
//                    generate_3d: crate::fbm::fbm_3d()
//                }
//            }
//        }
//        return traverse(value);
//    }
//}
