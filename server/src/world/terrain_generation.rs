use fmc::{
    blocks::Blocks,
    noise::Noise,
    prelude::*,
    world::{chunk::Chunk, TerrainGenerator},
};
use rand::SeedableRng;

use super::biomes::Biomes;

// The heighest point relative to the base height 3d noise can extend to create terrain.
const MAX_HEIGHT: i32 = 120;

// TODO: Read this from biome
// y_offset is the amount of blocks above the chunk that need to be generated to know how
// deep we are, in order to know which blocks to use when at the surface.
const Y_OFFSET: usize = 4;

pub struct Earth {
    biomes: Biomes,
    continents: Noise,
    terrain_height: Noise,
    terrain_shape: Noise,
    caves: Noise,
    seed: i32,
}

impl Earth {
    pub fn new(seed: i32, blocks: &Blocks) -> Self {
        //let freq = 1.0/200.0;
        //let terrain_low = Noise::simplex(0.0, seed).with_frequency(freq, 0.0, freq).fbm(4, 0.5, 2.0).mul_value(0.3);
        //let terrain_high = Noise::simplex(0.0, seed + 1).with_frequency(freq, 0.0, freq).fbm(4, 0.5, 2.0).max(terrain_low.clone());
        //let freq = 1.0/200.0;
        //let terrain_shape_low = Noise::simplex(0.0, seed + 2).with_frequency(freq, freq * 0.5, freq).fbm(5, 0.5, 2.0);
        //let terrain_shape_high = Noise::simplex(0.0, seed + 3).with_frequency(freq, freq * 0.5, freq).fbm(5, 0.5, 2.0);
        //let terrain_shape = Noise::simplex(0.0, seed + 4).with_frequency(freq, freq * 0.5, freq).lerp(terrain_shape_high, terrain_shape_low).range(0.1, -0.1, terrain_high, terrain_low);

        // ANOTHER ATTEMPT
        //let freq = 0.002;
        //let base_terrain = Noise::simplex(0.0, seed).with_frequency(freq, 0.0, freq).fbm(4, 0.5, 2.0).mul_value(0.1);
        //let freq = 0.003;
        //let mound = Noise::simplex(0.0, seed + 1).with_frequency(freq, 0.0, freq).fbm(4, 0.5, 2.0).abs().mul_value(0.3).add(base_terrain.clone()).max(base_terrain.clone());
        ////let mounds = Noise::simplex(0.005, seed + 3).fbm(4, 0.5, 2.0).range(0.5, -0.5, mound_high.clone(), mound_low.clone());

        ////let terrain_low = Noise::simplex(0.001, seed + 4).fbm(6, 0.5, 2.0).add(base_terrain.clone());
        ////let terrain_high = Noise::simplex(0.005, seed + 5).fbm(4, 0.5, 2.0).range(0.5, -0.5, mound_high, mound_low).add(base_terrain).add_value(0.5);

        //let freq = 1.0/150.0;
        //let terrain_shape = Noise::simplex(0.0, seed + 6).with_frequency(freq, freq * 0.5, freq).fbm(5, 0.5, 2.0);
        //let terrain_shape = terrain_shape.clone().range(0.5, -0.0, mound.clone(), base_terrain);
        ////let terrain_shape = terrain_shape.range(0.8, 0.7, mound.clone().add_value(0.4), terrain_shape_low);

        let freq = 0.005;
        let continents = Noise::perlin(freq, seed)
            .with_frequency(freq, 0.0, freq)
            .fbm(6, 0.5, 2.0)
            // Increase so less of the world is sea
            .add_value(0.25)
            // Reduce height of contintents to be between -10%/5% of MAX_HEIGHT
            .clamp(-0.1, 0.05);

        let freq = 1.0 / 128.0;
        let terrain_height = Noise::perlin(freq, seed + 1)
            .with_frequency(freq, 0.0, freq)
            .fbm(5, 0.5, 2.0)
            // Increase so less of the terrain is flat
            .add_value(0.5)
            // Move to range 0.5..1.5, see application for how it works
            .clamp(0.0, 1.0)
            .add_value(0.5);

        // When out at sea bottom out the terrain height gradually from the shore, so big
        // landmasses don't poke out.
        let terrain_height =
            continents
                .clone()
                .range(0.0, -0.05, terrain_height, Noise::constant(0.5));

        let freq = 1.0 / 2.0f32.powi(8);
        let high = Noise::perlin(freq, seed + 2).fbm(4, 0.5, 2.0);
        let low = Noise::perlin(freq, seed + 3).fbm(4, 0.5, 2.0);

        // High and low are switched between to create sudden changes in terrain elevation.
        //let freq = 1.0/92.0;
        let freq = 1.0 / 2.0f32.powi(9);
        let terrain_shape = Noise::perlin(freq, seed + 4)
            .fbm(8, 0.5, 2.0)
            .range(0.1, -0.1, high, low)
            .mul_value(2.0);

        // This is a failed attempt at making snaking tunnels. The idea is to generate 2d noise,
        // abs it, then use the values under some threshold as the direction of the tunnels. To
        // translate it into 3d, a 3d noise is generated through the same procedure, and overlayed
        // on the 2d noise. When you take the absolute value of 3d noise and threshold it, it
        // creates sheets, instead of lines. The overlay between the sheets and the lines of the 2d
        // noise create the tunnels, where the 2d noise effectively constitute the range
        // between the horizontal walls, and the 3d noise the range between the vertical walls.
        //
        // The big problems with this approach is one, no matter which depth you're at, the 2d noise
        // stays the same, and two, the 3d noise creates vertical walls when it changes direction,
        // when the 2d noise is parallel with these walls, it creates really tall narrow
        // unwalkable crevices.
        //
        //let freq = 0.004;
        //let tunnels = Noise::perlin(0.0, seed + 5)
        //    .with_frequency(freq * 2.0, freq * 2.0, freq * 2.0)
        //    .abs()
        //    .max(
        //        Noise::simplex(0.00, seed + 6)
        //            .with_frequency(freq, 0.0, freq)
        //            .abs()
        //    );

        // Visualization: https://www.shadertoy.com/view/stccDB
        let freq = 0.01;
        let cave_main = Noise::perlin(freq, seed + 5)
            .with_frequency(freq, freq * 2.0, freq)
            .fbm(3, 0.5, 2.0)
            .square();
        let cave_main_2 = Noise::perlin(freq, seed + 6)
            .with_frequency(freq, freq * 2.0, freq)
            .fbm(3, 0.5, 2.0)
            .square();
        let caves = continents.clone().range(
            // TODO: These numbers are slightly below the continents max because I implemented
            // range as non-inclusive.
            0.049,
            0.049,
            cave_main.add(cave_main_2),
            Noise::constant(1.0),
        );

        Self {
            biomes: Biomes::load(blocks),
            continents,
            terrain_height,
            terrain_shape,
            caves,
            seed,
        }
    }

    fn generate_terrain(&self, chunk_position: IVec3, chunk: &mut Chunk) {
        let (mut terrain_shape, _, _) = self.terrain_shape.generate_3d(
            chunk_position.x as f32,
            chunk_position.y as f32,
            chunk_position.z as f32,
            Chunk::SIZE,
            Chunk::SIZE + Y_OFFSET,
            Chunk::SIZE,
        );

        let (base_height, _, _) = self.continents.generate_3d(
            chunk_position.x as f32,
            0.0,
            chunk_position.z as f32,
            Chunk::SIZE,
            1,
            Chunk::SIZE,
        );

        let (terrain_height, _, _) = self.terrain_height.generate_3d(
            chunk_position.x as f32,
            0.0,
            chunk_position.z as f32,
            Chunk::SIZE,
            1,
            Chunk::SIZE,
        );

        for x in 0..Chunk::SIZE {
            for z in 0..Chunk::SIZE {
                let index = x << 4 | z;
                let base_height = base_height[index] * MAX_HEIGHT as f32;
                let terrain_height = terrain_height[index];
                for y in 0..Chunk::SIZE + Y_OFFSET {
                    // Amount the density should be decreased by per block above the base height
                    // for the maximum height to be MAX_HEIGHT.
                    // MAX_HEIGHT * DECREMENT / terrain_height_max = 1
                    const DECREMENT: f32 = 1.5 / MAX_HEIGHT as f32;
                    let mut compression = ((chunk_position.y + y as i32) as f32 - base_height)
                        * DECREMENT
                        / terrain_height;
                    if compression < 0.0 {
                        // Below surface, extra compression
                        compression *= 3.0;
                    }
                    let index = x * (Chunk::SIZE * (Chunk::SIZE + Y_OFFSET))
                        + z * (Chunk::SIZE + Y_OFFSET)
                        + y;
                    // Decrease density if above base height, increase if below
                    terrain_shape[index] -= compression;
                }
            }
        }

        chunk.blocks = vec![0; Chunk::SIZE.pow(3)];

        let biome = self.biomes.get_biome();

        for x in 0..Chunk::SIZE {
            for z in 0..Chunk::SIZE {
                let mut layer = 0;

                let base_height = base_height[x << 4 | z] * MAX_HEIGHT as f32;

                // Find how deep we are from above chunk.
                for y in Chunk::SIZE..Chunk::SIZE + Y_OFFSET {
                    // TODO: This needs to be converted to order xzy in simdnoise fork to make all
                    // access contiguous.
                    let block_index = x * (Chunk::SIZE * (Chunk::SIZE + Y_OFFSET))
                        + z * (Chunk::SIZE + Y_OFFSET)
                        + y;
                    let density = terrain_shape[block_index];

                    if density <= 0.0 {
                        if chunk_position.y + y as i32 <= 0 {
                            // For water
                            layer = 1;
                        }
                        break;
                    } else {
                        layer += 1;
                    }
                }

                for y in (0..Chunk::SIZE).rev() {
                    let block_height = chunk_position.y + y as i32;

                    let block_index = x * (Chunk::SIZE * (Chunk::SIZE + Y_OFFSET))
                        + z * (Chunk::SIZE + Y_OFFSET)
                        + y;
                    let density = terrain_shape[block_index];

                    let block = if density <= 0.0 {
                        if block_height == 0 {
                            layer = 1;
                            biome.surface_liquid
                        } else if block_height < 0 {
                            layer = 1;
                            biome.sub_surface_liquid
                        } else {
                            layer = 0;
                            biome.air
                        }
                    } else if layer > 3 {
                        layer += 1;
                        biome.bottom_layer_block
                    } else if block_height < 2 && base_height < 2.0 {
                        layer += 1;
                        biome.sand
                    } else {
                        let block = if layer < 1 {
                            biome.top_layer_block
                        } else if layer < 3 {
                            biome.mid_layer_block
                        } else {
                            biome.bottom_layer_block
                        };
                        layer += 1;
                        block
                    };

                    chunk[[x, y, z]] = block;
                }
            }
        }
    }

    fn carve_caves(&self, chunk_position: IVec3, chunk: &mut Chunk) {
        let air = Blocks::get().get_id("air");

        let biome = self.biomes.get_biome();
        let (caves, _, _) = self.caves.generate_3d(
            chunk_position.x as f32,
            chunk_position.y as f32,
            chunk_position.z as f32,
            Chunk::SIZE,
            Chunk::SIZE,
            Chunk::SIZE,
        );
        caves
            .into_iter()
            .zip(chunk.blocks.iter_mut())
            .enumerate()
            .for_each(|(i, (mut density, block))| {
                // TODO: Caves and water do not cooperate well. You carve the surface without
                // knowing there's water there and you get reverse moon pools underwater. Instead
                // we just push the caves underground, causing there to be no cave entraces at the
                // surface. There either needs to be a way to exclude caves from being generated
                // beneath water, or some way to intelligently fill carved out space that touches
                // water.
                const DECAY_POINT: i32 = -32;
                let y = chunk_position.y + (i & 0b1111) as i32;
                let density_offset = (y - DECAY_POINT).max(0) as f32 * 1.0 / 64.0;
                density += density_offset;

                if (density / 2.0) < 0.001
                    && *block != biome.surface_liquid
                    && *block != biome.sub_surface_liquid
                {
                    *block = air;
                }
            });
    }

    fn generate_features(&self, chunk_position: IVec3, chunk: &mut Chunk) {
        // TODO: It should be unique to each chunk but I don't know how.
        let seed = self
            .seed
            .overflowing_add(chunk_position.x.pow(2))
            .0
            .overflowing_add(chunk_position.z)
            .0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);

        let air = Blocks::get().get_id("air");

        // TODO: This should be done at terrain generation, but it clutters the code and it's in
        // flux. Meanwhile, it is done here. An entire extra scan of the chunk, and it can't tell
        // if it's the surface if it's the topmost block in a column.
        //
        // The surface contains the first block from the top that is not air for each block column
        // of the chunk.
        let mut surface = vec![None; Chunk::SIZE.pow(2)];
        for (column_index, block_column) in chunk.blocks.chunks(Chunk::SIZE).enumerate() {
            let mut air_encountered = false;
            for (y_index, block_id) in block_column.into_iter().enumerate().rev() {
                if air_encountered && *block_id != air {
                    // The 2d surface stores the index in the 3d chunk and the block. The
                    // bitshifting just converts it to a chunk index. See 'Chunk::Index' if
                    // wondering what it means.
                    surface[column_index] = Some((y_index, *block_id));
                    break;
                }
                if *block_id == air {
                    air_encountered = true;
                }
            }
        }

        let biome = self.biomes.get_biome();

        for blueprint in biome.blueprints.iter() {
            let terrain_feature = blueprint.construct(chunk_position, &surface, &mut rng);

            if terrain_feature.blocks.is_empty() {
                continue;
            }

            terrain_feature.apply(chunk, chunk_position);

            chunk.terrain_features.push(terrain_feature);
        }
    }
}

impl TerrainGenerator for Earth {
    // TODO: This takes ~1ms, way too slow. The simd needs to be inlined, the function call
    // overhead is 99% of the execution time I'm guessing. When initially benchmarking the noise
    // lib I remember using a simple 'add(some_value)' spiked execution time by 33/50%, it
    // corresponds to one extra simd instruction, compared to the hundreds of instructions of the
    // noise it is applied to.
    fn generate_chunk(&self, chunk_position: IVec3) -> Chunk {
        let mut chunk = Chunk::default();

        let air = Blocks::get().get_id("air");
        if MAX_HEIGHT < chunk_position.y {
            // Don't waste time generating if it is guaranteed to be air.
            chunk.make_uniform(air);
        } else {
            self.generate_terrain(chunk_position, &mut chunk);

            // TODO: Might make sense to test against water too.
            //
            // Test for air chunk uniformity early so we can break and elide the other generation
            // functions. This makes it so all other chunks that are uniform with another type of
            // block get stored as full size chunks. They are assumed to be very rare.
            let mut uniform = true;
            for block in chunk.blocks.iter() {
                if *block != air {
                    uniform = false;
                    break;
                }
            }

            if uniform {
                chunk.make_uniform(air);
                return chunk;
            }

            self.carve_caves(chunk_position, &mut chunk);
            self.generate_features(chunk_position, &mut chunk);
        }

        return chunk;
    }
}
