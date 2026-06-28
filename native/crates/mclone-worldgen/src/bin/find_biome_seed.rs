use mclone_worldgen::biome::{OverworldBiomeSource, possible_overworld_biomes};
use mclone_worldgen::block::{GRASS_BLOCK, RawBlockId, is_air_like, is_lava, is_water};
use mclone_worldgen::levelgen::{GeneratedChunk, NoiseBasedChunkGenerator, NoiseGeneratorSettings};

const DEFAULT_START_SEED: i64 = 0;
const DEFAULT_MAX_SEEDS: u64 = 100_000;
const DEFAULT_COUNT: usize = 1;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const DEFAULT_BIOME_RADIUS: i32 = 0;
const DEFAULT_SURFACE_RADIUS: i32 = 0;
const MAX_COUNT: usize = 1_000;
const MAX_SCORE_RADIUS: i32 = 8;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    if config.list_biomes {
        print_biomes();
        return Ok(());
    }

    let target_biome = config
        .biome_key
        .as_deref()
        .ok_or_else(|| format!("--biome is required\n{}", usage()))?;
    let matches = find_matches(&config, target_biome)?;
    print_matches(&config, target_biome, &matches);
    if matches.is_empty() {
        return Err(format!(
            "no seed found for {target_biome} at chunk ({}, {}) after searching {} seeds from {}",
            config.chunk_x, config.chunk_z, config.max_seeds, config.start_seed
        ));
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Config {
    biome_key: Option<String>,
    start_seed: i64,
    max_seeds: u64,
    count: usize,
    chunk_x: i32,
    chunk_z: i32,
    biome_radius: i32,
    min_biome_ratio: f64,
    surface_radius: i32,
    min_dry_surface_ratio: f64,
    min_grass_surface_ratio: f64,
    rank_by_score: bool,
    legacy_biome_init_layer: bool,
    large_biomes: bool,
    list_biomes: bool,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            biome_key: None,
            start_seed: DEFAULT_START_SEED,
            max_seeds: DEFAULT_MAX_SEEDS,
            count: DEFAULT_COUNT,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            biome_radius: DEFAULT_BIOME_RADIUS,
            min_biome_ratio: 0.0,
            surface_radius: DEFAULT_SURFACE_RADIUS,
            min_dry_surface_ratio: 0.0,
            min_grass_surface_ratio: 0.0,
            rank_by_score: false,
            legacy_biome_init_layer: false,
            large_biomes: false,
            list_biomes: false,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--biome" => {
                    config.biome_key = Some(normalize_biome_key(&next_arg(&mut args, "--biome")?));
                }
                "--start-seed" => config.start_seed = parse_next(&mut args, "--start-seed")?,
                "--max-seeds" => config.max_seeds = parse_next(&mut args, "--max-seeds")?,
                "--count" => config.count = parse_next(&mut args, "--count")?,
                "--chunk-x" => config.chunk_x = parse_next(&mut args, "--chunk-x")?,
                "--chunk-z" => config.chunk_z = parse_next(&mut args, "--chunk-z")?,
                "--biome-radius" => config.biome_radius = parse_next(&mut args, "--biome-radius")?,
                "--min-biome-ratio" => {
                    config.min_biome_ratio = parse_ratio(&mut args, "--min-biome-ratio")?;
                }
                "--surface-radius" => {
                    config.surface_radius = parse_next(&mut args, "--surface-radius")?;
                }
                "--min-dry-surface-ratio" => {
                    config.min_dry_surface_ratio =
                        parse_ratio(&mut args, "--min-dry-surface-ratio")?;
                }
                "--min-grass-surface-ratio" => {
                    config.min_grass_surface_ratio =
                        parse_ratio(&mut args, "--min-grass-surface-ratio")?;
                }
                "--rank-by-score" => config.rank_by_score = true,
                "--legacy-biome-init-layer" => config.legacy_biome_init_layer = true,
                "--large-biomes" => config.large_biomes = true,
                "--list-biomes" => config.list_biomes = true,
                "--" => {}
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument {arg}\n{}", usage())),
            }
        }

        if config.max_seeds == 0 {
            return Err("--max-seeds must be at least 1".to_owned());
        }
        if !(1..=MAX_COUNT).contains(&config.count) {
            return Err(format!("--count must be between 1 and {MAX_COUNT}"));
        }
        validate_radius("--biome-radius", config.biome_radius)?;
        validate_radius("--surface-radius", config.surface_radius)?;

        Ok(config)
    }

    fn uses_surface_quality(&self) -> bool {
        self.surface_radius > 0
            || self.min_dry_surface_ratio > 0.0
            || self.min_grass_surface_ratio > 0.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Match {
    seed: i64,
    biome_key: &'static str,
    score: SeedScore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeedScore {
    biome_radius: i32,
    matching_biome_chunks: usize,
    total_biome_chunks: usize,
    surface_radius: i32,
    dry_surface_columns: usize,
    grass_surface_columns: usize,
    total_surface_columns: usize,
}

impl SeedScore {
    const fn biome_only(
        biome_radius: i32,
        matching_biome_chunks: usize,
        total_biome_chunks: usize,
    ) -> Self {
        Self {
            biome_radius,
            matching_biome_chunks,
            total_biome_chunks,
            surface_radius: 0,
            dry_surface_columns: 0,
            grass_surface_columns: 0,
            total_surface_columns: 0,
        }
    }

    const fn with_surface(
        mut self,
        surface_radius: i32,
        dry_surface_columns: usize,
        grass_surface_columns: usize,
        total_surface_columns: usize,
    ) -> Self {
        self.surface_radius = surface_radius;
        self.dry_surface_columns = dry_surface_columns;
        self.grass_surface_columns = grass_surface_columns;
        self.total_surface_columns = total_surface_columns;
        self
    }

    fn biome_match_ratio(&self) -> f64 {
        ratio(self.matching_biome_chunks, self.total_biome_chunks)
    }

    fn dry_surface_ratio(&self) -> f64 {
        ratio(self.dry_surface_columns, self.total_surface_columns)
    }

    fn grass_surface_ratio(&self) -> f64 {
        ratio(self.grass_surface_columns, self.total_surface_columns)
    }

    fn quality_tuple(&self) -> (usize, usize, usize) {
        (
            self.matching_biome_chunks,
            self.dry_surface_columns,
            self.grass_surface_columns,
        )
    }
}

fn find_matches(config: &Config, target_biome: &str) -> Result<Vec<Match>, String> {
    let mut matches = Vec::new();
    for offset in 0..config.max_seeds {
        let offset = i64::try_from(offset)
            .map_err(|_| "--max-seeds is too large for i64 seed scanning".to_owned())?;
        let seed = config
            .start_seed
            .checked_add(offset)
            .ok_or_else(|| "seed search overflowed i64".to_owned())?;
        let biome_source =
            OverworldBiomeSource::new(seed, config.legacy_biome_init_layer, config.large_biomes);
        let biome = biome_source.get_primary_biome_definition(config.chunk_x, config.chunk_z);
        if biome.key() != target_biome {
            continue;
        }

        let mut score = score_biome_neighborhood(&biome_source, config, target_biome);
        if score.biome_match_ratio() + f64::EPSILON < config.min_biome_ratio {
            continue;
        }

        if config.uses_surface_quality() {
            score = score_surface(seed, config, score);
            if score.dry_surface_ratio() + f64::EPSILON < config.min_dry_surface_ratio
                || score.grass_surface_ratio() + f64::EPSILON < config.min_grass_surface_ratio
            {
                continue;
            }
        }

        matches.push(Match {
            seed,
            biome_key: biome.key(),
            score,
        });
        if !config.rank_by_score && matches.len() >= config.count {
            break;
        }
    }

    if config.rank_by_score {
        matches.sort_by(|left, right| {
            right
                .score
                .quality_tuple()
                .cmp(&left.score.quality_tuple())
                .then_with(|| left.seed.cmp(&right.seed))
        });
        matches.truncate(config.count);
    }

    Ok(matches)
}

fn score_biome_neighborhood(
    biome_source: &OverworldBiomeSource,
    config: &Config,
    target_biome: &str,
) -> SeedScore {
    let mut matching = 0;
    let mut total = 0;
    for chunk_z in config.chunk_z - config.biome_radius..=config.chunk_z + config.biome_radius {
        for chunk_x in config.chunk_x - config.biome_radius..=config.chunk_x + config.biome_radius {
            total += 1;
            if biome_source
                .get_primary_biome_definition(chunk_x, chunk_z)
                .key()
                == target_biome
            {
                matching += 1;
            }
        }
    }
    SeedScore::biome_only(config.biome_radius, matching, total)
}

fn score_surface(seed: i64, config: &Config, score: SeedScore) -> SeedScore {
    let mut dry_surface_columns = 0;
    let mut grass_surface_columns = 0;
    let mut total_surface_columns = 0;

    for chunk_z in config.chunk_z - config.surface_radius..=config.chunk_z + config.surface_radius {
        for chunk_x in
            config.chunk_x - config.surface_radius..=config.chunk_x + config.surface_radius
        {
            let chunk = generate_surface_chunk(seed, chunk_x, chunk_z, config);
            for local_z in 0..GeneratedChunk::WIDTH {
                for local_x in 0..GeneratedChunk::WIDTH {
                    total_surface_columns += 1;
                    if let Some(block_id) = top_non_air_block(&chunk, local_x, local_z) {
                        if !is_water(block_id) && !is_lava(block_id) {
                            dry_surface_columns += 1;
                        }
                        if block_id == GRASS_BLOCK {
                            grass_surface_columns += 1;
                        }
                    }
                }
            }
        }
    }

    score.with_surface(
        config.surface_radius,
        dry_surface_columns,
        grass_surface_columns,
        total_surface_columns,
    )
}

fn generate_surface_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    config: &Config,
) -> GeneratedChunk {
    let biome_source =
        OverworldBiomeSource::new(seed, config.legacy_biome_init_layer, config.large_biomes);
    let generator =
        NoiseBasedChunkGenerator::new(biome_source, seed, NoiseGeneratorSettings::overworld());
    let mut chunk = generator.fill_from_noise(chunk_x, chunk_z);
    generator.build_surface_and_bedrock(&mut chunk);
    GeneratedChunk::from_mutable_buffer(chunk)
}

fn top_non_air_block(chunk: &GeneratedChunk, local_x: i32, local_z: i32) -> Option<RawBlockId> {
    for y in (chunk.min_y..chunk.min_y + chunk.height).rev() {
        let block_id = chunk.block_at_y(local_x, y, local_z).raw();
        if !is_air_like(block_id) {
            return Some(block_id);
        }
    }
    None
}

fn print_matches(config: &Config, target_biome: &str, matches: &[Match]) {
    println!("{{");
    println!("  \"target_biome\": \"{target_biome}\",");
    println!("  \"chunk_x\": {},", config.chunk_x);
    println!("  \"chunk_z\": {},", config.chunk_z);
    println!("  \"start_seed\": {},", config.start_seed);
    println!("  \"searched_seeds_limit\": {},", config.max_seeds);
    println!("  \"biome_radius\": {},", config.biome_radius);
    println!("  \"min_biome_ratio\": {:.3},", config.min_biome_ratio);
    println!("  \"surface_radius\": {},", config.surface_radius);
    println!(
        "  \"min_dry_surface_ratio\": {:.3},",
        config.min_dry_surface_ratio
    );
    println!(
        "  \"min_grass_surface_ratio\": {:.3},",
        config.min_grass_surface_ratio
    );
    println!("  \"rank_by_score\": {},", config.rank_by_score);
    println!(
        "  \"legacy_biome_init_layer\": {},",
        config.legacy_biome_init_layer
    );
    println!("  \"large_biomes\": {},", config.large_biomes);
    println!("  \"matches\": [");
    for (index, matched) in matches.iter().enumerate() {
        let comma = if index + 1 == matches.len() { "" } else { "," };
        println!("    {{");
        println!("      \"seed\": {},", matched.seed);
        println!("      \"chunk_x\": {},", config.chunk_x);
        println!("      \"chunk_z\": {},", config.chunk_z);
        println!("      \"primary_biome\": \"{}\",", matched.biome_key);
        print_score(matched.score);
        println!("    }}{comma}");
    }
    println!("  ]");
    println!("}}");
}

fn print_score(score: SeedScore) {
    println!("      \"quality\": {{");
    println!("        \"biome_radius\": {},", score.biome_radius);
    println!(
        "        \"matching_biome_chunks\": {},",
        score.matching_biome_chunks
    );
    println!(
        "        \"total_biome_chunks\": {},",
        score.total_biome_chunks
    );
    println!(
        "        \"biome_match_ratio\": {:.3},",
        score.biome_match_ratio()
    );
    println!("        \"surface_radius\": {},", score.surface_radius);
    println!(
        "        \"dry_surface_columns\": {},",
        score.dry_surface_columns
    );
    println!(
        "        \"grass_surface_columns\": {},",
        score.grass_surface_columns
    );
    println!(
        "        \"total_surface_columns\": {},",
        score.total_surface_columns
    );
    println!(
        "        \"dry_surface_ratio\": {:.3},",
        score.dry_surface_ratio()
    );
    println!(
        "        \"grass_surface_ratio\": {:.3}",
        score.grass_surface_ratio()
    );
    println!("      }}");
}

fn print_biomes() {
    for biome in possible_overworld_biomes() {
        println!("{}", biome.key());
    }
}

fn normalize_biome_key(value: &str) -> String {
    if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    }
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    let value = next_arg(args, name)?;
    value
        .parse::<T>()
        .map_err(|_| format!("invalid value for {name}: {value}"))
}

fn parse_ratio(args: &mut impl Iterator<Item = String>, name: &str) -> Result<f64, String> {
    let value = parse_next::<f64>(args, name)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(format!("{name} must be between 0.0 and 1.0"));
    }
    Ok(value)
}

fn validate_radius(name: &str, radius: i32) -> Result<(), String> {
    if !(0..=MAX_SCORE_RADIUS).contains(&radius) {
        return Err(format!("{name} must be between 0 and {MAX_SCORE_RADIUS}"));
    }
    Ok(())
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn usage() -> String {
    "usage: find_biome_seed --biome <key> [--start-seed N] [--max-seeds N] [--count N] [--chunk-x N] [--chunk-z N] [--biome-radius N] [--min-biome-ratio 0..1] [--surface-radius N] [--min-dry-surface-ratio 0..1] [--min-grass-surface-ratio 0..1] [--rank-by-score] [--legacy-biome-init-layer] [--large-biomes]\n       find_biome_seed --list-biomes"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_biome_keys_adds_default_namespace() {
        assert_eq!(normalize_biome_key("plains"), "minecraft:plains");
        assert_eq!(normalize_biome_key("minecraft:desert"), "minecraft:desert");
    }

    #[test]
    fn parse_defaults_to_zero_zero_seed_search() {
        let config = Config::parse(["--biome", "plains"].into_iter().map(str::to_owned)).unwrap();

        assert_eq!(
            config,
            Config {
                biome_key: Some("minecraft:plains".to_owned()),
                start_seed: 0,
                max_seeds: DEFAULT_MAX_SEEDS,
                count: DEFAULT_COUNT,
                chunk_x: 0,
                chunk_z: 0,
                biome_radius: DEFAULT_BIOME_RADIUS,
                min_biome_ratio: 0.0,
                surface_radius: DEFAULT_SURFACE_RADIUS,
                min_dry_surface_ratio: 0.0,
                min_grass_surface_ratio: 0.0,
                rank_by_score: false,
                legacy_biome_init_layer: false,
                large_biomes: false,
                list_biomes: false,
            }
        );
    }

    #[test]
    fn parse_quality_search_flags() {
        let config = Config::parse(
            [
                "--biome",
                "plains",
                "--biome-radius",
                "2",
                "--min-biome-ratio",
                "0.6",
                "--surface-radius",
                "1",
                "--min-dry-surface-ratio",
                "0.8",
                "--min-grass-surface-ratio",
                "0.5",
                "--rank-by-score",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();

        assert_eq!(config.biome_radius, 2);
        assert_eq!(config.min_biome_ratio, 0.6);
        assert_eq!(config.surface_radius, 1);
        assert_eq!(config.min_dry_surface_ratio, 0.8);
        assert_eq!(config.min_grass_surface_ratio, 0.5);
        assert!(config.rank_by_score);
    }

    #[test]
    fn parse_rejects_out_of_range_quality_values() {
        assert!(
            Config::parse(
                ["--biome", "plains", "--min-biome-ratio", "1.1"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
        assert!(
            Config::parse(
                ["--biome", "plains", "--surface-radius", "9"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
    }

    #[test]
    fn find_plains_seed_at_origin_from_small_range() {
        let config = Config {
            biome_key: Some("minecraft:plains".to_owned()),
            start_seed: 0,
            max_seeds: 20,
            count: 1,
            chunk_x: 0,
            chunk_z: 0,
            biome_radius: 0,
            min_biome_ratio: 0.0,
            surface_radius: 0,
            min_dry_surface_ratio: 0.0,
            min_grass_surface_ratio: 0.0,
            rank_by_score: false,
            legacy_biome_init_layer: false,
            large_biomes: false,
            list_biomes: false,
        };

        let matches = find_matches(&config, "minecraft:plains").unwrap();
        assert_eq!(
            matches,
            vec![Match {
                seed: 16,
                biome_key: "minecraft:plains",
                score: SeedScore::biome_only(0, 1, 1),
            }]
        );
    }
}
