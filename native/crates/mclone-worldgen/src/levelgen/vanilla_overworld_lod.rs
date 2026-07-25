use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use crate::biome::{BiomeDefinition, OverworldBiomeSource};
use crate::block::{AIR, WATER, material_blocks_motion};
use crate::surface::overworld_surface_top_material;

use super::{NoiseBasedChunkGenerator, NoiseGeneratorSettings};

pub const VANILLA_OVERWORLD_LOD_REVISION: &str = "vanilla-1.17.1-density-column-lod-v1";
pub const VANILLA_OVERWORLD_LOD_MAX_RETAINED_DENSITY_COLUMNS: usize = 4_096;
pub const VANILLA_OVERWORLD_MACRO_LOD_REVISION: &str =
    "vanilla-1.17.1-sparse-density-column-lod-v1";
pub const VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VanillaOverworldLodSample {
    pub solid_surface_y: i32,
    pub display_y: i32,
    pub water: bool,
    pub biome: BiomeDefinition,
    pub approximate_surface_material: u8,
    pub visible_material: u8,
}

pub struct VanillaOverworldLodSampler {
    seed: i64,
    generator: NoiseBasedChunkGenerator<OverworldBiomeSource>,
    biome_source: OverworldBiomeSource,
    density_columns: BTreeMap<(i32, i32), Arc<[f64]>>,
    insertion_order: VecDeque<(i32, i32)>,
    generated_density_columns: u64,
    reused_density_columns: u64,
}

impl VanillaOverworldLodSampler {
    pub fn new(seed: i64) -> Self {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        Self {
            seed,
            generator: NoiseBasedChunkGenerator::new(
                biome_source.clone(),
                seed,
                NoiseGeneratorSettings::overworld(),
            ),
            biome_source,
            density_columns: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            generated_density_columns: 0,
            reused_density_columns: 0,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn retained_density_columns(&self) -> usize {
        self.density_columns.len()
    }

    pub const fn generated_density_columns(&self) -> u64 {
        self.generated_density_columns
    }

    pub const fn reused_density_columns(&self) -> u64 {
        self.reused_density_columns
    }

    pub fn clear_cache(&mut self) {
        self.density_columns.clear();
        self.insertion_order.clear();
    }

    pub fn sample(&mut self, world_x: i32, world_z: i32) -> VanillaOverworldLodSample {
        let cell_width = self.generator.lod_cell_width();
        let cell_x = world_x.div_euclid(cell_width);
        let cell_z = world_z.div_euclid(cell_width);
        let x_fraction = f64::from(world_x.rem_euclid(cell_width)) / f64::from(cell_width);
        let z_fraction = f64::from(world_z.rem_euclid(cell_width)) / f64::from(cell_width);
        let x0z0 = self.density_column(cell_x, cell_z);
        let x0z1 = (z_fraction != 0.0).then(|| self.density_column(cell_x, cell_z + 1));
        let x1z0 = (x_fraction != 0.0).then(|| self.density_column(cell_x + 1, cell_z));
        let x1z1 = (x_fraction != 0.0 && z_fraction != 0.0)
            .then(|| self.density_column(cell_x + 1, cell_z + 1));
        sample_density_surface(
            &self.generator,
            &self.biome_source,
            self.seed,
            world_x,
            world_z,
            1,
            &x0z0,
            x0z1.as_deref(),
            x1z0.as_deref(),
            x1z1.as_deref(),
        )
    }

    fn density_column(&mut self, cell_x: i32, cell_z: i32) -> Arc<[f64]> {
        let key = (cell_x, cell_z);
        if let Some(column) = self.density_columns.get(&key) {
            self.reused_density_columns = self.reused_density_columns.saturating_add(1);
            return Arc::clone(column);
        }
        while self.density_columns.len() >= VANILLA_OVERWORLD_LOD_MAX_RETAINED_DENSITY_COLUMNS {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.density_columns.remove(&oldest);
        }
        let mut values = vec![0.0; (self.generator.lod_cell_count_y() + 1) as usize];
        self.generator
            .fill_lod_noise_column(cell_x, cell_z, &mut values);
        let values = Arc::<[f64]>::from(values);
        self.density_columns.insert(key, Arc::clone(&values));
        self.insertion_order.push_back(key);
        self.generated_density_columns = self.generated_density_columns.saturating_add(1);
        values
    }
}

pub struct VanillaOverworldMacroSampler {
    seed: i64,
    generator: NoiseBasedChunkGenerator<OverworldBiomeSource>,
    biome_source: OverworldBiomeSource,
    density_columns: BTreeMap<(i32, i32), Arc<[f64]>>,
    insertion_order: VecDeque<(i32, i32)>,
    generated_density_columns: u64,
    reused_density_columns: u64,
}

impl VanillaOverworldMacroSampler {
    pub fn new(seed: i64) -> Self {
        let biome_source = OverworldBiomeSource::new(seed, false, false);
        Self {
            seed,
            generator: NoiseBasedChunkGenerator::new(
                biome_source.clone(),
                seed,
                NoiseGeneratorSettings::overworld(),
            ),
            biome_source,
            density_columns: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            generated_density_columns: 0,
            reused_density_columns: 0,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn retained_density_columns(&self) -> usize {
        self.density_columns.len()
    }

    pub const fn generated_density_columns(&self) -> u64 {
        self.generated_density_columns
    }

    pub const fn reused_density_columns(&self) -> u64 {
        self.reused_density_columns
    }

    pub fn clear_cache(&mut self) {
        self.density_columns.clear();
        self.insertion_order.clear();
    }

    pub fn sample(&mut self, world_x: i32, world_z: i32) -> VanillaOverworldLodSample {
        let cell_width = self.generator.lod_cell_width();
        let cell_x = world_x.div_euclid(cell_width);
        let cell_z = world_z.div_euclid(cell_width);
        let x_fraction = f64::from(world_x.rem_euclid(cell_width)) / f64::from(cell_width);
        let z_fraction = f64::from(world_z.rem_euclid(cell_width)) / f64::from(cell_width);
        let x0z0 = self.density_column(cell_x, cell_z);
        let x0z1 = (z_fraction != 0.0).then(|| self.density_column(cell_x, cell_z + 1));
        let x1z0 = (x_fraction != 0.0).then(|| self.density_column(cell_x + 1, cell_z));
        let x1z1 = (x_fraction != 0.0 && z_fraction != 0.0)
            .then(|| self.density_column(cell_x + 1, cell_z + 1));
        sample_density_surface(
            &self.generator,
            &self.biome_source,
            self.seed,
            world_x,
            world_z,
            VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP,
            &x0z0,
            x0z1.as_deref(),
            x1z0.as_deref(),
            x1z1.as_deref(),
        )
    }

    fn density_column(&mut self, cell_x: i32, cell_z: i32) -> Arc<[f64]> {
        let key = (cell_x, cell_z);
        if let Some(column) = self.density_columns.get(&key) {
            self.reused_density_columns = self.reused_density_columns.saturating_add(1);
            return Arc::clone(column);
        }
        while self.density_columns.len() >= VANILLA_OVERWORLD_LOD_MAX_RETAINED_DENSITY_COLUMNS {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.density_columns.remove(&oldest);
        }
        let sparse_cell_count =
            self.generator.lod_cell_count_y() / VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP;
        let mut values = vec![0.0; (sparse_cell_count + 1) as usize];
        self.generator.fill_lod_sparse_noise_column(
            cell_x,
            cell_z,
            VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP,
            &mut values,
        );
        let values = Arc::<[f64]>::from(values);
        self.density_columns.insert(key, Arc::clone(&values));
        self.insertion_order.push_back(key);
        self.generated_density_columns = self.generated_density_columns.saturating_add(1);
        values
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_density_surface(
    generator: &NoiseBasedChunkGenerator<OverworldBiomeSource>,
    biome_source: &OverworldBiomeSource,
    seed: i64,
    world_x: i32,
    world_z: i32,
    vertical_cell_step: i32,
    x0z0: &[f64],
    x0z1: Option<&[f64]>,
    x1z0: Option<&[f64]>,
    x1z1: Option<&[f64]>,
) -> VanillaOverworldLodSample {
    let cell_width = generator.lod_cell_width();
    let x_fraction = f64::from(world_x.rem_euclid(cell_width)) / f64::from(cell_width);
    let z_fraction = f64::from(world_z.rem_euclid(cell_width)) / f64::from(cell_width);
    let cell_height = generator.lod_cell_height() * vertical_cell_step;
    let min_cell_y = generator.lod_min_cell_y();
    let sparse_cell_count = generator.lod_cell_count_y() / vertical_cell_step;
    let mut display_y = generator.lod_min_y();
    let mut solid_surface_y = generator.lod_min_y();
    let mut visible_material = AIR;

    'cells: for cell_y in (0..sparse_cell_count).rev() {
        let y_index = cell_y as usize;
        for y_offset in (0..cell_height).rev() {
            let y_fraction = f64::from(y_offset) / f64::from(cell_height);
            let x0z0_density = lerp(y_fraction, x0z0[y_index], x0z0[y_index + 1]);
            let z0_density = x1z0.map_or(x0z0_density, |x1z0| {
                lerp(
                    x_fraction,
                    x0z0_density,
                    lerp(y_fraction, x1z0[y_index], x1z0[y_index + 1]),
                )
            });
            let density = x0z1.map_or(z0_density, |x0z1| {
                let x0z1_density = lerp(y_fraction, x0z1[y_index], x0z1[y_index + 1]);
                let z1_density = x1z1.map_or(x0z1_density, |x1z1| {
                    lerp(
                        x_fraction,
                        x0z1_density,
                        lerp(y_fraction, x1z1[y_index], x1z1[y_index + 1]),
                    )
                });
                lerp(z_fraction, z0_density, z1_density)
            });
            let block_y =
                (min_cell_y + cell_y * vertical_cell_step) * generator.lod_cell_height() + y_offset;
            let block = generator.resolve_terrain_block(block_y, density);
            if display_y == generator.lod_min_y() && block != AIR {
                display_y = block_y + 1;
                visible_material = block;
            }
            if material_blocks_motion(block) {
                solid_surface_y = block_y + 1;
                break 'cells;
            }
        }
    }

    let biome = biome_source.get_block_position_biome_definition(seed, world_x, world_z);
    let approximate_surface_material = overworld_surface_top_material(biome);
    let water = visible_material == WATER;
    VanillaOverworldLodSample {
        solid_surface_y,
        display_y,
        water,
        biome,
        approximate_surface_material,
        visible_material: if water {
            WATER
        } else {
            approximate_surface_material
        },
    }
}

fn lerp(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}

#[cfg(test)]
mod tests {
    use mclone_core::{ChunkPos, local_block_coord};

    use crate::block::{WATER, material_blocks_motion};
    use crate::levelgen::{
        GeneratedChunk, MutableChunkBlockBuffer, generate_overworld_surface_chunk,
    };

    use super::*;

    #[test]
    fn direct_samples_match_full_noise_columns() {
        for (seed, positions) in [
            (
                12_345,
                [
                    (0, 0),
                    (3, 3),
                    (4, 4),
                    (15, 15),
                    (16, 16),
                    (-1, -1),
                    (-4, 7),
                    (-17, 31),
                ],
            ),
            (
                -98_765,
                [
                    (-304, 336),
                    (-301, 339),
                    (80, 1_840),
                    (95, 1_855),
                    (96, 1_856),
                    (2_048, -2_049),
                    (-8_193, 4_097),
                    (32_767, -32_768),
                ],
            ),
        ] {
            let biome_source = OverworldBiomeSource::new(seed, false, false);
            let generator = NoiseBasedChunkGenerator::new(
                biome_source,
                seed,
                NoiseGeneratorSettings::overworld(),
            );
            let mut sampler = VanillaOverworldLodSampler::new(seed);
            let mut chunks = BTreeMap::new();
            for (world_x, world_z) in positions {
                let chunk_pos = ChunkPos::from_block_coords(world_x, world_z);
                let chunk = chunks
                    .entry(chunk_pos)
                    .or_insert_with(|| generator.fill_from_noise(chunk_pos.x, chunk_pos.z));
                let actual = sampler.sample(world_x, world_z);
                let local_x = local_block_coord(world_x);
                let local_z = local_block_coord(world_z);
                assert_eq!(
                    actual.display_y,
                    column_height(chunk, local_x, local_z, |block| block != AIR),
                    "world surface at seed {seed}, ({world_x}, {world_z})"
                );
                assert_eq!(
                    actual.solid_surface_y,
                    column_height(chunk, local_x, local_z, material_blocks_motion),
                    "ocean floor at seed {seed}, ({world_x}, {world_z})"
                );
                assert_eq!(
                    actual.water,
                    top_block(chunk, local_x, local_z) == WATER,
                    "water at seed {seed}, ({world_x}, {world_z})"
                );
            }
        }
    }

    #[test]
    fn sample_cache_is_optional_and_reuses_lattice_columns() {
        let mut sampler = VanillaOverworldLodSampler::new(12_345);
        let first = sampler.sample(-1, -1);
        let generated = sampler.generated_density_columns();
        assert_eq!(generated, 4);
        let second = sampler.sample(-1, -1);
        assert_eq!(first, second);
        assert_eq!(sampler.generated_density_columns(), generated);
        assert_eq!(sampler.reused_density_columns(), 4);

        sampler.clear_cache();
        let cold = sampler.sample(-1, -1);
        assert_eq!(cold, first);
        assert_eq!(sampler.retained_density_columns(), 4);
    }

    #[test]
    fn sample_generates_only_density_columns_with_nonzero_weight() {
        let mut aligned = VanillaOverworldLodSampler::new(12_345);
        aligned.sample(-4, 8);
        assert_eq!(aligned.generated_density_columns(), 1);

        let mut x_interior = VanillaOverworldLodSampler::new(12_345);
        x_interior.sample(-2, 8);
        assert_eq!(x_interior.generated_density_columns(), 2);

        let mut z_interior = VanillaOverworldLodSampler::new(12_345);
        z_interior.sample(-4, 10);
        assert_eq!(z_interior.generated_density_columns(), 2);

        let mut interior = VanillaOverworldLodSampler::new(12_345);
        interior.sample(-2, 10);
        assert_eq!(interior.generated_density_columns(), 4);
    }

    #[test]
    fn macro_sample_is_deterministic_and_reuses_sparse_columns() {
        let mut sampler = VanillaOverworldMacroSampler::new(-98_765);
        let first = sampler.sample(-301, 339);
        let generated = sampler.generated_density_columns();
        assert_eq!(generated, 4);
        let repeated = sampler.sample(-301, 339);

        assert_eq!(first, repeated);
        assert_eq!(sampler.generated_density_columns(), generated);
        assert_eq!(sampler.reused_density_columns(), 4);
        assert_eq!(
            sampler.density_columns.values().next().unwrap().len(),
            (sampler.generator.lod_cell_count_y() / VANILLA_OVERWORLD_MACRO_VERTICAL_CELL_STEP + 1)
                as usize
        );

        sampler.clear_cache();
        assert_eq!(sampler.sample(-301, 339), first);
    }

    #[test]
    fn approximate_material_does_not_claim_surface_builder_parity() {
        let mut sampler = VanillaOverworldLodSampler::new(12_345);
        let sample = sampler.sample(0, 0);
        let surface = generate_overworld_surface_chunk(12_345, 0, 0);
        let local_x = local_block_coord(0);
        let local_z = local_block_coord(0);
        let actual_top = top_solid_block(&surface, local_x, local_z);

        assert_ne!(sample.approximate_surface_material, AIR);
        assert!(actual_top != AIR);
    }

    fn column_height(
        chunk: &MutableChunkBlockBuffer,
        local_x: i32,
        local_z: i32,
        predicate: impl Fn(u8) -> bool,
    ) -> i32 {
        for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
            if predicate(chunk.get_block_at_y(local_x, y, local_z)) {
                return y + 1;
            }
        }
        chunk.min_y
    }

    fn top_block(chunk: &MutableChunkBlockBuffer, local_x: i32, local_z: i32) -> u8 {
        let height = column_height(chunk, local_x, local_z, |block| block != AIR);
        if height == chunk.min_y {
            AIR
        } else {
            chunk.get_block_at_y(local_x, height - 1, local_z)
        }
    }

    fn top_solid_block(chunk: &GeneratedChunk, local_x: i32, local_z: i32) -> u8 {
        for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
            let block = chunk.block_at_y(local_x, y, local_z).raw();
            if material_blocks_motion(block) {
                return block;
            }
        }
        AIR
    }
}
